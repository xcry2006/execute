// Feature: production-ready-improvements, Property 21: 环境变量传递
// **Validates: Requirements 14.3**
//
// 属性 21: 环境变量传递
// 对于任意设置的环境变量，子进程应该能访问到相同的值（往返测试）
//
// 验证需求：
// - 需求 14.3: WHEN 执行命令时，THE System SHALL 将配置的环境变量传递给子进程

use execute::{CommandConfig, EnvConfig};
use proptest::prelude::*;
use std::collections::HashMap;

/// 生成有效的环境变量名策略
/// 环境变量名应该：
/// - 以字母或下划线开头
/// - 只包含字母、数字和下划线
/// - 长度在 1-50 之间
fn env_var_name_strategy() -> impl Strategy<Value = String> {
    use proptest::char;
    
    // 第一个字符：字母或下划线
    let first_char = prop_oneof![
        char::range('A', 'Z').prop_map(|c| c.to_string()),
        char::range('a', 'z').prop_map(|c| c.to_string()),
        Just("_".to_string()),
    ];

    // 后续字符：字母、数字或下划线
    let rest_chars = prop::collection::vec(
        prop_oneof![
            char::range('A', 'Z'),
            char::range('a', 'z'),
            char::range('0', '9'),
            Just('_'),
        ],
        0..=49,
    )
    .prop_map(|chars| chars.into_iter().collect::<String>());

    (first_char, rest_chars).prop_map(|(first, rest)| format!("{}{}", first, rest))
}

/// 生成环境变量值策略
/// 值可以包含任意可打印字符，但避免换行符以简化测试
fn env_var_value_strategy() -> impl Strategy<Value = String> {
    use proptest::char;
    
    // 使用可打印的 ASCII 字符，避免换行符和特殊字符
    prop::collection::vec(
        prop_oneof![
            char::range('a', 'z'),
            char::range('A', 'Z'),
            char::range('0', '9'),
            Just(' '),
            Just('-'),
            Just('_'),
            Just('.'),
            Just('/'),
            Just(':'),
        ],
        0..=100,
    )
    .prop_map(|chars| chars.into_iter().collect::<String>())
}

/// 生成环境变量映射策略（1-5个变量）
fn env_vars_strategy() -> impl Strategy<Value = HashMap<String, String>> {
    prop::collection::hash_map(env_var_name_strategy(), env_var_value_strategy(), 1..=5)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 属性测试：对于任意设置的环境变量，子进程应该能访问到相同的值
    ///
    /// 此测试使用往返测试方法：
    /// 1. 生成随机的环境变量键值对
    /// 2. 通过 EnvConfig 设置这些变量
    /// 3. 执行一个命令来打印这些环境变量
    /// 4. 验证输出的值与设置的值完全匹配
    ///
    /// 验证需求：
    /// - 需求 14.3: 执行命令时，系统应将配置的环境变量传递给子进程
    #[test]
    fn prop_env_var_passing_round_trip(
        env_vars in env_vars_strategy()
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 创建 EnvConfig 并设置所有环境变量
        let mut env_config = EnvConfig::new();
        for (key, value) in &env_vars {
            env_config = env_config.set(key, value);
        }

        // 对于每个环境变量，执行命令来验证其值
        for (key, expected_value) in &env_vars {
            // 使用 printenv 命令打印特定环境变量的值
            // printenv VAR_NAME 会输出该变量的值
            let config = CommandConfig::new("printenv", vec![key.clone()])
                .with_env(env_config.clone());

            // 执行命令
            let result = execute::execute_with_retry(&config, 1);

            // 验证命令成功执行
            prop_assert!(
                result.is_ok(),
                "Command should succeed for env var '{}': {:?}",
                key,
                result.err()
            );

            let output = result.unwrap();
            prop_assert!(
                output.status.success(),
                "Exit status should be success for env var '{}'",
                key
            );

            // 验证输出值与设置的值匹配
            let actual_value = String::from_utf8_lossy(&output.stdout);
            // Remove only the trailing newline, not all whitespace
            let actual_value = actual_value.strip_suffix('\n')
                .or_else(|| actual_value.strip_suffix("\r\n"))
                .unwrap_or(&actual_value);
            
            prop_assert_eq!(
                actual_value,
                expected_value.as_str(),
                "Environment variable '{}' should have value '{}' but got '{}'",
                key,
                expected_value,
                actual_value
            );
        }
    }

    /// 属性测试：对于任意设置的多个环境变量，所有变量都应该正确传递
    ///
    /// 此测试验证多个环境变量同时设置时的正确性
    ///
    /// 验证需求：
    /// - 需求 14.3: 执行命令时，系统应将配置的环境变量传递给子进程
    #[test]
    fn prop_multiple_env_vars_passing(
        env_vars in env_vars_strategy()
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 创建 EnvConfig 并设置所有环境变量
        let mut env_config = EnvConfig::new();
        for (key, value) in &env_vars {
            env_config = env_config.set(key, value);
        }

        // 使用 env 命令打印所有环境变量
        // 然后用 grep 过滤我们设置的变量
        let config = CommandConfig::new("env", vec![])
            .with_env(env_config.clone());

        // 执行命令
        let result = execute::execute_with_retry(&config, 2);

        // 验证命令成功执行
        prop_assert!(
            result.is_ok(),
            "Command should succeed: {:?}",
            result.err()
        );

        let output = result.unwrap();
        prop_assert!(
            output.status.success(),
            "Exit status should be success"
        );

        // 解析输出，构建实际的环境变量映射
        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut actual_vars = HashMap::new();
        for line in output_str.lines() {
            if let Some((key, value)) = line.split_once('=') {
                actual_vars.insert(key.to_string(), value.to_string());
            }
        }

        // 验证所有设置的环境变量都存在且值正确
        for (key, expected_value) in &env_vars {
            prop_assert!(
                actual_vars.contains_key(key),
                "Environment variable '{}' should be present in output",
                key
            );

            let actual_value = &actual_vars[key];
            prop_assert_eq!(
                actual_value,
                expected_value,
                "Environment variable '{}' should have value '{}' but got '{}'",
                key,
                expected_value,
                actual_value
            );
        }
    }

    /// 属性测试：环境变量值可以包含特殊字符
    ///
    /// 验证需求：
    /// - 需求 14.3: 执行命令时，系统应将配置的环境变量传递给子进程
    #[test]
    fn prop_env_var_with_special_chars(
        key in env_var_name_strategy(),
        value in env_var_value_strategy()
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 创建 EnvConfig 并设置环境变量
        let env_config = EnvConfig::new().set(&key, &value);

        // 使用 printenv 命令打印环境变量的值
        let config = CommandConfig::new("printenv", vec![key.clone()])
            .with_env(env_config);

        // 执行命令
        let result = execute::execute_with_retry(&config, 3);

        // 验证命令成功执行
        prop_assert!(
            result.is_ok(),
            "Command should succeed: {:?}",
            result.err()
        );

        let output = result.unwrap();
        prop_assert!(
            output.status.success(),
            "Exit status should be success"
        );

        // 验证输出值与设置的值匹配
        let actual_value = String::from_utf8_lossy(&output.stdout);
        // Remove only the trailing newline, not all whitespace
        let actual_value = actual_value.strip_suffix('\n')
            .or_else(|| actual_value.strip_suffix("\r\n"))
            .unwrap_or(&actual_value);
        
        prop_assert_eq!(
            actual_value,
            value.as_str(),
            "Environment variable '{}' should have value '{}' but got '{}'",
            key,
            value,
            actual_value
        );
    }
}

// 单元测试：验证特定场景

#[test]
#[cfg(unix)]
fn test_single_env_var_passing() {
    // 测试单个环境变量的传递
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    let env_config = EnvConfig::new().set("TEST_VAR", "test_value");

    let config = CommandConfig::new("printenv", vec!["TEST_VAR".to_string()])
        .with_env(env_config);

    let result = execute::execute_with_retry(&config, 1);

    assert!(result.is_ok(), "Command should succeed");
    let output = result.unwrap();
    assert!(output.status.success(), "Exit status should be success");

    let actual_value = String::from_utf8_lossy(&output.stdout);
    let actual_value = actual_value.strip_suffix('\n')
        .or_else(|| actual_value.strip_suffix("\r\n"))
        .unwrap_or(&actual_value);
    assert_eq!(actual_value, "test_value");
}

#[test]
#[cfg(unix)]
fn test_multiple_env_vars_passing() {
    // 测试多个环境变量的传递
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    let env_config = EnvConfig::new()
        .set("VAR1", "value1")
        .set("VAR2", "value2")
        .set("VAR3", "value3");

    // 验证每个变量
    for (key, expected) in [("VAR1", "value1"), ("VAR2", "value2"), ("VAR3", "value3")] {
        let config = CommandConfig::new("printenv", vec![key.to_string()])
            .with_env(env_config.clone());

        let result = execute::execute_with_retry(&config, 2);
        assert!(result.is_ok(), "Command should succeed for {}", key);

        let output = result.unwrap();
        assert!(output.status.success(), "Exit status should be success for {}", key);

        let actual_value = String::from_utf8_lossy(&output.stdout);
        let actual_value = actual_value.strip_suffix('\n')
            .or_else(|| actual_value.strip_suffix("\r\n"))
            .unwrap_or(&actual_value);
        assert_eq!(actual_value, expected, "Value mismatch for {}", key);
    }
}

#[test]
#[cfg(unix)]
fn test_env_var_with_spaces() {
    // 测试包含空格的环境变量值
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    let env_config = EnvConfig::new().set("TEST_VAR", "value with spaces");

    let config = CommandConfig::new("printenv", vec!["TEST_VAR".to_string()])
        .with_env(env_config);

    let result = execute::execute_with_retry(&config, 3);

    assert!(result.is_ok(), "Command should succeed");
    let output = result.unwrap();
    assert!(output.status.success(), "Exit status should be success");

    let actual_value = String::from_utf8_lossy(&output.stdout);
    let actual_value = actual_value.strip_suffix('\n')
        .or_else(|| actual_value.strip_suffix("\r\n"))
        .unwrap_or(&actual_value);
    assert_eq!(actual_value, "value with spaces");
}

#[test]
#[cfg(unix)]
fn test_env_var_with_special_characters() {
    // 测试包含特殊字符的环境变量值
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    let env_config = EnvConfig::new().set("TEST_VAR", "path/to/file:123");

    let config = CommandConfig::new("printenv", vec!["TEST_VAR".to_string()])
        .with_env(env_config);

    let result = execute::execute_with_retry(&config, 4);

    assert!(result.is_ok(), "Command should succeed");
    let output = result.unwrap();
    assert!(output.status.success(), "Exit status should be success");

    let actual_value = String::from_utf8_lossy(&output.stdout);
    let actual_value = actual_value.strip_suffix('\n')
        .or_else(|| actual_value.strip_suffix("\r\n"))
        .unwrap_or(&actual_value);
    assert_eq!(actual_value, "path/to/file:123");
}

#[test]
#[cfg(unix)]
fn test_env_var_empty_value() {
    // 测试空值的环境变量
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    let env_config = EnvConfig::new().set("TEST_VAR", "");

    let config = CommandConfig::new("printenv", vec!["TEST_VAR".to_string()])
        .with_env(env_config);

    let result = execute::execute_with_retry(&config, 5);

    assert!(result.is_ok(), "Command should succeed");
    let output = result.unwrap();
    assert!(output.status.success(), "Exit status should be success");

    let actual_value = String::from_utf8_lossy(&output.stdout);
    let actual_value = actual_value.strip_suffix('\n')
        .or_else(|| actual_value.strip_suffix("\r\n"))
        .unwrap_or(&actual_value);
    assert_eq!(actual_value, "");
}

#[test]
#[cfg(unix)]
fn test_env_var_overwrite_existing() {
    // 测试覆盖现有环境变量
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 使用 USER 或 HOME 这样的变量，而不是 PATH
    // 因为覆盖 PATH 会导致无法找到命令
    let custom_value = "custom_test_value";
    let env_config = EnvConfig::new().set("USER", custom_value);

    let config = CommandConfig::new("printenv", vec!["USER".to_string()])
        .with_env(env_config);

    let result = execute::execute_with_retry(&config, 6);

    assert!(result.is_ok(), "Command should succeed");
    let output = result.unwrap();
    assert!(output.status.success(), "Exit status should be success");

    let actual_value = String::from_utf8_lossy(&output.stdout);
    let actual_value = actual_value.strip_suffix('\n')
        .or_else(|| actual_value.strip_suffix("\r\n"))
        .unwrap_or(&actual_value);
    assert_eq!(actual_value, custom_value);
}

#[test]
#[cfg(unix)]
fn test_env_var_inherit_parent() {
    // 测试继承父进程环境变量
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 设置一个新变量，同时继承父进程的环境变量
    let env_config = EnvConfig::new().set("NEW_VAR", "new_value");

    // PATH 应该从父进程继承
    let config = CommandConfig::new("printenv", vec!["PATH".to_string()])
        .with_env(env_config);

    let result = execute::execute_with_retry(&config, 7);

    assert!(result.is_ok(), "Command should succeed");
    let output = result.unwrap();
    assert!(output.status.success(), "Exit status should be success");

    // PATH 应该存在（从父进程继承）
    let actual_value = String::from_utf8_lossy(&output.stdout);
    let actual_value = actual_value.strip_suffix('\n')
        .or_else(|| actual_value.strip_suffix("\r\n"))
        .unwrap_or(&actual_value);
    assert!(!actual_value.is_empty(), "PATH should be inherited from parent");
}

#[test]
#[cfg(unix)]
fn test_env_var_long_value() {
    // 测试长值的环境变量
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    let long_value = "a".repeat(1000);
    let env_config = EnvConfig::new().set("TEST_VAR", &long_value);

    let config = CommandConfig::new("printenv", vec!["TEST_VAR".to_string()])
        .with_env(env_config);

    let result = execute::execute_with_retry(&config, 8);

    assert!(result.is_ok(), "Command should succeed");
    let output = result.unwrap();
    assert!(output.status.success(), "Exit status should be success");

    let actual_value = String::from_utf8_lossy(&output.stdout);
    let actual_value = actual_value.strip_suffix('\n')
        .or_else(|| actual_value.strip_suffix("\r\n"))
        .unwrap_or(&actual_value);
    assert_eq!(actual_value, long_value);
}
