use std::time::Duration;

use reqwest::StatusCode;

/// DEFAULT_MAX_RETRIES 表示默认最大重试次数。
pub const DEFAULT_MAX_RETRIES: usize = 3;

/// RetryPolicy 定义请求失败后的重试次数和退避策略。
#[derive(Clone, Debug)]
pub struct RetryPolicy {
    pub max_retries: usize,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub jitter: bool,
}

impl Default for RetryPolicy {
    /// 创建默认重试策略。
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            base_delay: Duration::from_millis(300),
            max_delay: Duration::from_secs(5),
            jitter: true,
        }
    }
}

impl RetryPolicy {
    /// 创建指定重试次数的策略。
    pub fn new(max_retries: usize) -> Self {
        Self {
            max_retries,
            ..Self::default()
        }
    }

    /// 设置基础退避时间。
    pub fn with_base_delay(mut self, base_delay: Duration) -> Self {
        self.base_delay = base_delay;
        self
    }

    /// 设置最大退避时间。
    pub fn with_max_delay(mut self, max_delay: Duration) -> Self {
        self.max_delay = max_delay;
        self
    }

    /// 设置是否启用轻量抖动。
    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }

    /// 判断某个状态码是否应该重试。
    pub fn should_retry_status(&self, status: StatusCode) -> bool {
        status == StatusCode::REQUEST_TIMEOUT
            || status == StatusCode::TOO_MANY_REQUESTS
            || status.is_server_error()
    }

    /// 计算第 n 次重试前的等待时间。
    pub fn delay_for_attempt(&self, attempt: usize) -> Duration {
        let multiplier = 2_u32.saturating_pow(attempt as u32);
        let mut delay = self.base_delay.saturating_mul(multiplier);

        if self.jitter {
            let jitter = self.base_delay.as_millis() as u64 / 2;
            delay = delay.saturating_add(Duration::from_millis(jitter));
        }

        delay.min(self.max_delay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// 验证可重试状态码识别逻辑。
    fn retry_policy_detects_retryable_status() {
        let policy = RetryPolicy::default();

        assert_eq!(policy.max_retries, DEFAULT_MAX_RETRIES);
        assert!(policy.should_retry_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(policy.should_retry_status(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(!policy.should_retry_status(StatusCode::NOT_FOUND));
    }

    #[test]
    /// 验证重试等待时间不会超过上限。
    fn retry_delay_is_capped() {
        let policy = RetryPolicy::new(3)
            .with_base_delay(Duration::from_secs(2))
            .with_max_delay(Duration::from_secs(3))
            .with_jitter(false);

        assert_eq!(policy.delay_for_attempt(3), Duration::from_secs(3));
    }
}
