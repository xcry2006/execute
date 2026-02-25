//! 环境变量应用优化模块
//!
//! 提供高效的环境变量应用实现，减少内存分配和系统调用。
//!
//! # 优化策略
//!
//! 1. **缓存环境变量块**：预先生成环境变量数组，避免重复计算
//! 2. **批量应用**：使用 `envs()` 一次性应用多个环境变量
//! 3. **避免临时字符串**：使用 `&str` 而非 `String` 减少分配
//! 4. **延迟清除**：只在必要时调用 `env_clear()`

use std::collections::HashMap;
use std::process::Command;

use crate::config::EnvConfig;

/// 优化的环境变量应用器
///
/// 缓存环境变量配置，支持高效批量应用。
pub struct EnvOptimizer {
    /// 是否继承父进程环境
    inherit_parent: bool,
    /// 设置的环境变量（键值对）
    vars_to_set: Vec<(String, String)>,
    /// 要清除的环境变量
    vars_to_remove: Vec<String>,
    /// 缓存的 C 字符串格式（用于 execve）
    cached_envp: Option<Vec<std::ffi::CString>>,
}

impl EnvOptimizer {
    /// 从 EnvConfig 创建优化器
    ///
    /// 预解析配置，分类设置和清除操作。
    pub fn from_config(config: &EnvConfig) -> Self {
        let vars = config.vars();
        let mut vars_to_set = Vec::with_capacity(vars.len());
        let mut vars_to_remove = Vec::new();

        for (key, value) in vars {
            match value {
                Some(v) => {
                    vars_to_set.push((key.clone(), v.clone()));
                }
                None => {
                    vars_to_remove.push(key.clone());
                }
            }
        }

        Self {
            inherit_parent: config.inherit_parent(),
            vars_to_set,
            vars_to_remove,
            cached_envp: None,
        }
    }

    /// 应用到 Command
    ///
    /// 使用最优策略应用环境变量：
    /// - 如果 vars_to_set 较多，使用 `envs()` 批量设置
    /// - 如果 vars_to_remove 较多且 inherit_parent=false，直接 `env_clear()`
    /// - 否则逐个处理
    pub fn apply(&self, cmd: &mut Command) {
        // 策略 1: 不继承且全部清除，直接 env_clear
        if !self.inherit_parent && self.vars_to_set.is_empty() {
            cmd.env_clear();
            return;
        }

        // 策略 2: 不继承但有设置，先 clear 再批量设置
        if !self.inherit_parent {
            cmd.env_clear();
            // 使用 envs 批量设置
            let envs: Vec<(&str, &str)> = self
                .vars_to_set
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            cmd.envs(envs);
            return;
        }

        // 策略 3: 继承父进程，需要处理 set 和 remove
        // 先处理 remove
        for key in &self.vars_to_remove {
            cmd.env_remove(key);
        }

        // 批量设置
        if !self.vars_to_set.is_empty() {
            let envs: Vec<(&str, &str)> = self
                .vars_to_set
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            cmd.envs(envs);
        }
    }

    /// 快速应用（无缓存版本）
    ///
    /// 直接从 EnvConfig 应用，不创建优化器实例。
    /// 适合单次使用场景。
    pub fn apply_fast(cmd: &mut Command, config: &EnvConfig) {
        let vars = config.vars();

        // 不继承父进程环境
        if !config.inherit_parent() {
            cmd.env_clear();
        }

        // 收集需要设置的环境变量
        let envs: Vec<(&str, &str)> = vars
            .iter()
            .filter_map(|(k, v)| v.as_ref().map(|val| (k.as_str(), val.as_str())))
            .collect();

        // 批量设置
        if !envs.is_empty() {
            cmd.envs(envs);
        }

        // 处理需要清除的变量
        if config.inherit_parent() {
            for (key, value) in vars {
                if value.is_none() {
                    cmd.env_remove(key);
                }
            }
        }
    }

    /// 获取设置的环境变量数量
    pub fn set_count(&self) -> usize {
        self.vars_to_set.len()
    }

    /// 获取清除的环境变量数量
    pub fn remove_count(&self) -> usize {
        self.vars_to_remove.len()
    }

    /// 是否为空配置
    pub fn is_empty(&self) -> bool {
        self.vars_to_set.is_empty() && self.vars_to_remove.is_empty()
    }
}

/// 环境变量缓存
///
/// 用于频繁使用相同环境变量配置的场景。
pub struct EnvCache {
    configs: HashMap<u64, EnvOptimizer>,
}

impl EnvCache {
    /// 创建新的缓存
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
        }
    }

    /// 获取或创建优化器
    ///
    /// 使用配置的哈希值作为缓存键。
    pub fn get_or_create(&mut self, config: &EnvConfig) -> &EnvOptimizer {
        let hash = Self::hash_config(config);
        self.configs
            .entry(hash)
            .or_insert_with(|| EnvOptimizer::from_config(config))
    }

    /// 计算配置哈希
    fn hash_config(config: &EnvConfig) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        config.inherit_parent().hash(&mut hasher);
        
        let mut vars: Vec<_> = config.vars().iter().collect();
        vars.sort_by(|a, b| a.0.cmp(b.0));
        
        for (key, value) in vars {
            key.hash(&mut hasher);
            value.hash(&mut hasher);
        }
        
        hasher.finish()
    }

    /// 清空缓存
    pub fn clear(&mut self) {
        self.configs.clear();
    }

    /// 缓存大小
    pub fn len(&self) -> usize {
        self.configs.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.configs.is_empty()
    }
}

impl Default for EnvCache {
    fn default() -> Self {
        Self::new()
    }
}

/// 优化的 apply_env_config 函数
///
/// 替换原有的 apply_env_config，使用批量操作减少系统调用。
pub fn apply_env_config_optimized(cmd: &mut Command, config: &EnvConfig) {
    EnvOptimizer::apply_fast(cmd, config);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_optimizer_basic() {
        let config = EnvConfig::new()
            .set("KEY1", "value1")
            .set("KEY2", "value2");

        let optimizer = EnvOptimizer::from_config(&config);
        assert_eq!(optimizer.set_count(), 2);
        assert!(optimizer.remove_count() == 0);
    }

    #[test]
    fn test_env_optimizer_with_remove() {
        let config = EnvConfig::new()
            .set("KEY1", "value1")
            .remove("KEY2");

        let optimizer = EnvOptimizer::from_config(&config);
        assert_eq!(optimizer.set_count(), 1);
        assert_eq!(optimizer.remove_count(), 1);
    }

    #[test]
    fn test_env_cache() {
        let mut cache = EnvCache::new();
        
        let config1 = EnvConfig::new().set("A", "1");
        let config2 = EnvConfig::new().set("A", "1");
        
        cache.get_or_create(&config1);
        cache.get_or_create(&config2);
        
        // 相同配置应该共享缓存
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_apply_fast() {
        let config = EnvConfig::new()
            .no_inherit()
            .set("TEST_VAR", "test_value");

        let mut cmd = Command::new("echo");
        EnvOptimizer::apply_fast(&mut cmd, &config);
        
        // 验证可以通过编译和执行
        // 实际环境变量验证需要运行子进程
    }
}
