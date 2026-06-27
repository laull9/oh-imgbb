//! user_agents 模块提供内置 User-Agent 字符串查询能力。

use std::collections::HashMap;
use std::sync::OnceLock;

/// USER_AGENTS_DATA 保存从外部库移植的 User-Agent 数据。
const USER_AGENTS_DATA: &str = include_str!("../assets/user_agents.tsv");

/// USER_AGENTS 保存按需解析后的 User-Agent 索引。
static USER_AGENTS: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();

/// 返回全部 User-Agent 映射。
pub fn user_agents() -> &'static HashMap<&'static str, &'static str> {
    USER_AGENTS.get_or_init(parse_user_agents)
}

/// 按名称查询 User-Agent 字符串。
pub fn get(name: &str) -> Option<&'static str> {
    get_user_agent(name)
}

/// 按名称查询 User-Agent 字符串。
pub fn get_user_agent(name: &str) -> Option<&'static str> {
    user_agents().get(name).copied()
}

/// 判断指定名称是否存在。
pub fn has_user_agent(name: &str) -> bool {
    user_agents().contains_key(name)
}

/// 返回内置 User-Agent 数量。
pub fn user_agent_count() -> usize {
    user_agents().len()
}

/// 判断内置 User-Agent 集合是否为空。
pub fn user_agent_is_empty() -> bool {
    user_agents().is_empty()
}

/// 遍历全部 User-Agent 名称。
pub fn user_agent_names() -> impl Iterator<Item = &'static str> {
    user_agents().keys().copied()
}

/// 遍历全部 User-Agent 字符串。
pub fn user_agent_values() -> impl Iterator<Item = &'static str> {
    user_agents().values().copied()
}

/// 解析静态 User-Agent 数据。
fn parse_user_agents() -> HashMap<&'static str, &'static str> {
    let mut agents = HashMap::new();

    for line in USER_AGENTS_DATA.lines() {
        let Some((name, value)) = line.split_once('\t') else {
            continue;
        };

        if !name.is_empty() && !value.is_empty() {
            agents.insert(name, value);
        }
    }

    agents
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证可以按名称查询外部库中的 User-Agent。
    #[test]
    fn user_agents_get_known_value() {
        let agent = get("mozilla").unwrap();

        assert!(agent.contains("Mozilla/5.0"));
    }

    /// 验证移植数据包含完整规模的候选集合。
    #[test]
    fn user_agents_include_large_dataset() {
        assert!(user_agent_count() > 9_000);
        assert!(has_user_agent("firefox_2592"));
    }

    /// 验证名称和值迭代器可以读取数据。
    #[test]
    fn user_agents_iterators_return_entries() {
        assert!(user_agent_names().any(|name| name == "ie"));
        assert!(user_agent_values().any(|value| value.contains("Firefox")));
    }
}
