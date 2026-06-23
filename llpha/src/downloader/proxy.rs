use anyhow::Result;
use std::sync::{Arc, Mutex};

/// ProxyPool 为网络客户端提供可扩展的代理选择接口。
///
/// 实现方可以按轮询、权重、失败率或外部服务动态返回代理地址。
/// 返回值使用 reqwest 可识别的代理 URL，例如 `http://127.0.0.1:7890`。
pub trait ProxyPool: Send + Sync {
    /// 返回下一次请求可用的代理地址。
    fn next_proxy(&self) -> Result<Option<String>>;
}

/// InMemoryProxyPool 提供简单的内存轮询代理池。
#[derive(Clone, Debug)]
pub struct InMemoryProxyPool {
    proxies: Arc<Vec<String>>,
    cursor: Arc<Mutex<usize>>,
}

impl InMemoryProxyPool {
    /// 创建新的内存代理池。
    pub fn new(proxies: Vec<String>) -> Self {
        Self {
            proxies: Arc::new(proxies),
            cursor: Arc::new(Mutex::new(0)),
        }
    }

    /// 判断代理池是否为空。
    pub fn is_empty(&self) -> bool {
        self.proxies.is_empty()
    }
}

impl ProxyPool for InMemoryProxyPool {
    /// 返回下一个轮询代理地址。
    fn next_proxy(&self) -> Result<Option<String>> {
        if self.proxies.is_empty() {
            return Ok(None);
        }

        let mut cursor = self
            .cursor
            .lock()
            .map_err(|err| anyhow::anyhow!("代理池游标加锁失败: {err}"))?;
        let proxy = self.proxies[*cursor % self.proxies.len()].clone();
        *cursor = cursor.saturating_add(1);

        Ok(Some(proxy))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// 验证代理池会按轮询顺序返回代理。
    fn proxy_pool_round_robins_values() {
        let pool = InMemoryProxyPool::new(vec![
            "http://127.0.0.1:1001".to_string(),
            "http://127.0.0.1:1002".to_string(),
        ]);

        assert_eq!(
            pool.next_proxy().unwrap(),
            Some("http://127.0.0.1:1001".to_string())
        );
        assert_eq!(
            pool.next_proxy().unwrap(),
            Some("http://127.0.0.1:1002".to_string())
        );
        assert_eq!(
            pool.next_proxy().unwrap(),
            Some("http://127.0.0.1:1001".to_string())
        );
    }

    #[test]
    /// 验证空代理池返回空代理。
    fn proxy_pool_can_be_empty() {
        let pool = InMemoryProxyPool::new(vec![]);

        assert!(pool.is_empty());
        assert_eq!(pool.next_proxy().unwrap(), None);
    }
}
