// Feature: production-ready-improvements, Property 22: 环境变量清除
// **Validates: Requirements 14.5**
//
// 属性 22: 环境变量清除
// 对于任意标记为清除的环境变量，子进程不应该能访问到该变量
//
// 验证需求：
// - 需求 14.5: THE System SHALL 支持清除特定环境变量

use execute::{CommandConfig, EnvConfig};
use proptest::prelude::*;
use std::env;

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
fn env_var_value_strategy() -> impl Strategy<Value = String> {
    use proptest::char;

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
        1..=100,
    )
    .prop_map(|chars| chars.into_iter().collect::<String>())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 属性测试：对于任意标记为清除的环境变量，子进程不应该能访问到该变量
    ///
    /// 此测试验证环境变量清除功能：
    /// 1. 在父进程中设置一个环境变量
    /// 2. 使用 EnvConfig::remove() 标记该变量为清除
    /// 3. 执行命令尝试访问该变量
    /// 4. 验证子进程无法访问该变量（printenv 应该失败或返回空）
    ///
    /// 验证需求：
    /// - 需求 14.5: 系统应支持清除特定环境变量
    #[test]
    fn prop_env_var_clearing(
        var_name in env_var_name_strategy(),
        var_value in env_var_value_strategy()
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 在父进程中设置环境变量
        unsafe {
            env::set_var(&var_name, &var_value);
        }

        // 验证父进程中变量已设置
        let parent_value = env::var(&var_name).ok();
        prop_assert_eq!(
            parent_value.as_deref(),
            Some(var_value.as_str()),
            "Environment variable '{}' should be set in parent process",
            var_name
        );

        // 创建 EnvConfig 并标记该变量为清除
        let env_config = EnvConfig::new().remove(&var_name);

        // 使用 printenv 命令尝试打印该环境变量
        // 如果变量被清除，printenv 应该返回非零退出码
        let config = CommandConfig::new("printenv", vec![var_name.clone()])
            .with_env(env_config);

        // 执行命令
        let result = execute::execute_with_retry(&config, 1);

        // 清理：移除父进程中的环境变量
        unsafe {
            env::remove_var(&var_name);
        }

        // 验证命令执行成功（没有系统错误）
        prop_assert!(
            result.is_ok(),
            "Command execution should not fail with system error: {:?}",
            result.err()
        );

        let output = result.unwrap();

        // 验证 printenv 返回非零退出码（因为变量不存在）
        // 或者输出为空（某些系统上 printenv 可能返回 0 但输出为空）
        if output.status.success() {
            // 如果退出码为 0，输出应该为空
            let actual_output = String::from_utf8_lossy(&output.stdout);
            let actual_output = actual_output.trim();
            prop_assert!(
                actual_output.is_empty(),
                "Environment variable '{}' should not be accessible in child process, but got output: '{}'",
                var_name,
                actual_output
            );
        } else {
            // 退出码非零表示变量不存在，这是预期的
            // 这是正确的行为
        }
    }

    /// 属性测试：清除多个环境变量
    ///
    /// 验证需求：
    /// - 需求 14.5: 系统应支持清除特定环境变量
    #[test]
    fn prop_multiple_env_vars_clearing(
        var_names in prop::collection::vec(env_var_name_strategy(), 1..=5)
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 在父进程中设置所有环境变量
        for var_name in &var_names {
            unsafe {
                env::set_var(var_name, "test_value");
            }
        }

        // 创建 EnvConfig 并清除所有这些变量
        let mut env_config = EnvConfig::new();
        for var_name in &var_names {
            env_config = env_config.remove(var_name);
        }

        // 对每个变量，验证它们都被清除了
        for var_name in &var_names {
            let config = CommandConfig::new("printenv", vec![var_name.clone()])
                .with_env(env_config.clone());

            let result = execute::execute_with_retry(&config, 2);

            // 清理：移除父进程中的环境变量
            unsafe {
                env::remove_var(var_name);
            }

            prop_assert!(
                result.is_ok(),
                "Command execution should not fail with system error for '{}': {:?}",
                var_name,
                result.err()
            );

            let output = result.unwrap();

            // 验证变量不可访问
            if output.status.success() {
                let actual_output = String::from_utf8_lossy(&output.stdout);
                let actual_output = actual_output.trim();
                prop_assert!(
                    actual_output.is_empty(),
                    "Environment variable '{}' should not be accessible in child process",
                    var_name
                );
            }
        }
    }

    /// 属性测试：清除变量的同时设置其他变量
    ///
    /// 验证清除操作不影响其他变量的设置
    ///
    /// 验证需求：
    /// - 需求 14.5: 系统应支持清除特定环境变量
    #[test]
    fn prop_clearing_with_setting(
        clear_var in env_var_name_strategy(),
        set_var in env_var_name_strategy(),
        set_value in env_var_value_strategy()
    ) {
        // 确保两个变量名不同
        prop_assume!(clear_var != set_var);

        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 在父进程中设置要清除的变量
        unsafe {
            env::set_var(&clear_var, "should_be_cleared");
        }

        // 创建 EnvConfig：清除一个变量，设置另一个变量
        let env_config = EnvConfig::new()
            .remove(&clear_var)
            .set(&set_var, &set_value);

        // 验证清除的变量不可访问
        let config_clear = CommandConfig::new("printenv", vec![clear_var.clone()])
            .with_env(env_config.clone());

        let result_clear = execute::execute_with_retry(&config_clear, 3);

        // 清理
        unsafe {
            env::remove_var(&clear_var);
        }

        prop_assert!(
            result_clear.is_ok(),
            "Command execution should not fail: {:?}",
            result_clear.err()
        );

        let output_clear = result_clear.unwrap();
        if output_clear.status.success() {
            let actual_output = String::from_utf8_lossy(&output_clear.stdout);
            let actual_output = actual_output.trim();
            prop_assert!(
                actual_output.is_empty(),
                "Cleared variable '{}' should not be accessible",
                clear_var
            );
        }

        // 验证设置的变量可以访问
        let config_set = CommandConfig::new("printenv", vec![set_var.clone()])
            .with_env(env_config);

        let result_set = execute::execute_with_retry(&config_set, 4);

        prop_assert!(
            result_set.is_ok(),
            "Command execution should not fail: {:?}",
            result_set.err()
        );

        let output_set = result_set.unwrap();
        prop_assert!(
            output_set.status.success(),
            "Exit status should be success for set variable '{}'",
            set_var
        );

        let actual_value = String::from_utf8_lossy(&output_set.stdout);
        let actual_value = actual_value.strip_suffix('\n')
            .or_else(|| actual_value.strip_suffix("\r\n"))
            .unwrap_or(&actual_value);

        prop_assert_eq!(
            actual_value,
            set_value.as_str(),
            "Set variable '{}' should have correct value",
            set_var
        );
    }
}

// 单元测试：验证特定场景

#[test]
#[cfg(unix)]
fn test_clear_single_env_var() {
    // 测试清除单个环境变量
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 在父进程中设置变量
    unsafe {
        env::set_var("TEST_CLEAR_VAR", "should_not_exist");
    }

    // 创建配置清除该变量
    let env_config = EnvConfig::new().remove("TEST_CLEAR_VAR");

    let config =
        CommandConfig::new("printenv", vec!["TEST_CLEAR_VAR".to_string()]).with_env(env_config);

    let result = execute::execute_with_retry(&config, 1);

    // 清理
    unsafe {
        env::remove_var("TEST_CLEAR_VAR");
    }

    assert!(result.is_ok(), "Command should succeed");
    let output = result.unwrap();

    // printenv 应该返回非零退出码或空输出
    if output.status.success() {
        let actual_output = String::from_utf8_lossy(&output.stdout);
        assert!(
            actual_output.trim().is_empty(),
            "Cleared variable should not be accessible"
        );
    }
}

#[test]
#[cfg(unix)]
fn test_clear_multiple_env_vars() {
    // 测试清除多个环境变量
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 在父进程中设置变量
    unsafe {
        env::set_var("CLEAR_VAR1", "value1");
        env::set_var("CLEAR_VAR2", "value2");
        env::set_var("CLEAR_VAR3", "value3");
    }

    // 创建配置清除所有变量
    let env_config = EnvConfig::new()
        .remove("CLEAR_VAR1")
        .remove("CLEAR_VAR2")
        .remove("CLEAR_VAR3");

    // 验证每个变量都被清除
    for var_name in ["CLEAR_VAR1", "CLEAR_VAR2", "CLEAR_VAR3"] {
        let config =
            CommandConfig::new("printenv", vec![var_name.to_string()]).with_env(env_config.clone());

        let result = execute::execute_with_retry(&config, 2);

        // 清理
        unsafe {
            env::remove_var(var_name);
        }

        assert!(result.is_ok(), "Command should succeed for {}", var_name);
        let output = result.unwrap();

        if output.status.success() {
            let actual_output = String::from_utf8_lossy(&output.stdout);
            assert!(
                actual_output.trim().is_empty(),
                "Variable {} should not be accessible",
                var_name
            );
        }
    }
}

#[test]
#[cfg(unix)]
fn test_clear_and_set_different_vars() {
    // 测试同时清除和设置不同的变量
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 在父进程中设置要清除的变量
    unsafe {
        env::set_var("VAR_TO_CLEAR", "should_be_cleared");
    }

    // 创建配置：清除一个变量，设置另一个变量
    let env_config = EnvConfig::new()
        .remove("VAR_TO_CLEAR")
        .set("VAR_TO_SET", "new_value");

    // 验证清除的变量不可访问
    let config_clear = CommandConfig::new("printenv", vec!["VAR_TO_CLEAR".to_string()])
        .with_env(env_config.clone());

    let result_clear = execute::execute_with_retry(&config_clear, 3);

    // 清理
    unsafe {
        env::remove_var("VAR_TO_CLEAR");
    }

    assert!(result_clear.is_ok(), "Command should succeed");
    let output_clear = result_clear.unwrap();

    if output_clear.status.success() {
        let actual_output = String::from_utf8_lossy(&output_clear.stdout);
        assert!(
            actual_output.trim().is_empty(),
            "Cleared variable should not be accessible"
        );
    }

    // 验证设置的变量可以访问
    let config_set =
        CommandConfig::new("printenv", vec!["VAR_TO_SET".to_string()]).with_env(env_config);

    let result_set = execute::execute_with_retry(&config_set, 4);

    assert!(result_set.is_ok(), "Command should succeed");
    let output_set = result_set.unwrap();
    assert!(output_set.status.success(), "Exit status should be success");

    let actual_value = String::from_utf8_lossy(&output_set.stdout);
    let actual_value = actual_value
        .strip_suffix('\n')
        .or_else(|| actual_value.strip_suffix("\r\n"))
        .unwrap_or(&actual_value);
    assert_eq!(actual_value, "new_value");
}

#[test]
#[cfg(unix)]
fn test_clear_nonexistent_var() {
    // 测试清除不存在的变量（应该不会出错）
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 确保变量不存在
    unsafe {
        env::remove_var("NONEXISTENT_VAR");
    }

    // 创建配置清除不存在的变量
    let env_config = EnvConfig::new().remove("NONEXISTENT_VAR");

    let config =
        CommandConfig::new("printenv", vec!["NONEXISTENT_VAR".to_string()]).with_env(env_config);

    let result = execute::execute_with_retry(&config, 5);

    assert!(result.is_ok(), "Command should succeed");
    let output = result.unwrap();

    // printenv 应该返回非零退出码或空输出
    if output.status.success() {
        let actual_output = String::from_utf8_lossy(&output.stdout);
        assert!(
            actual_output.trim().is_empty(),
            "Nonexistent variable should not be accessible"
        );
    }
}

#[test]
#[cfg(unix)]
fn test_clear_with_no_inherit() {
    // 测试在不继承父进程环境变量的情况下清除变量
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 在父进程中设置变量
    unsafe {
        env::set_var("TEST_VAR", "parent_value");
    }

    // 创建配置：不继承父进程环境变量，并清除该变量
    let env_config = EnvConfig::new().no_inherit().remove("TEST_VAR");

    let config = CommandConfig::new("printenv", vec!["TEST_VAR".to_string()]).with_env(env_config);

    let result = execute::execute_with_retry(&config, 6);

    // 清理
    unsafe {
        env::remove_var("TEST_VAR");
    }

    assert!(result.is_ok(), "Command should succeed");
    let output = result.unwrap();

    // 变量应该不可访问
    if output.status.success() {
        let actual_output = String::from_utf8_lossy(&output.stdout);
        assert!(
            actual_output.trim().is_empty(),
            "Variable should not be accessible"
        );
    }
}

#[test]
#[cfg(unix)]
fn test_clear_path_like_var() {
    // 测试清除类似 PATH 的变量（包含路径分隔符）
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 在父进程中设置变量
    unsafe {
        env::set_var("TEST_PATH_VAR", "/usr/bin:/usr/local/bin");
    }

    // 创建配置清除该变量
    let env_config = EnvConfig::new().remove("TEST_PATH_VAR");

    let config =
        CommandConfig::new("printenv", vec!["TEST_PATH_VAR".to_string()]).with_env(env_config);

    let result = execute::execute_with_retry(&config, 7);

    // 清理
    unsafe {
        env::remove_var("TEST_PATH_VAR");
    }

    assert!(result.is_ok(), "Command should succeed");
    let output = result.unwrap();

    if output.status.success() {
        let actual_output = String::from_utf8_lossy(&output.stdout);
        assert!(
            actual_output.trim().is_empty(),
            "Cleared path variable should not be accessible"
        );
    }
}
