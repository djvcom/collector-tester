use std::time::Duration;

use async_trait::async_trait;
use mock_collector::ServerHandle;

#[async_trait]
pub trait CollectorAssertions {
    async fn assert_spans_received(&self, expected_count: usize, timeout: Duration);
    async fn assert_metrics_received(&self, expected_count: usize, timeout: Duration);
    async fn assert_logs_received(&self, expected_count: usize, timeout: Duration);

    async fn assert_span_exists(&self, span_name: &str, timeout: Duration);
    async fn assert_span_not_exists(&self, span_name: &str, wait_duration: Duration);

    async fn assert_span_has_attribute(
        &self,
        span_name: &str,
        attribute_key: &str,
        expected_value: &str,
        timeout: Duration,
    );

    async fn span_count(&self) -> usize;
    async fn metric_count(&self) -> usize;
    async fn log_count(&self) -> usize;
}

#[async_trait]
impl CollectorAssertions for ServerHandle {
    async fn assert_spans_received(&self, expected_count: usize, timeout: Duration) {
        self.wait_for_spans(expected_count, timeout)
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "timed out waiting for {} spans (timeout: {:?})",
                    expected_count, timeout
                )
            });
    }

    async fn assert_metrics_received(&self, expected_count: usize, timeout: Duration) {
        self.wait_for_metrics(expected_count, timeout)
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "timed out waiting for {} metrics (timeout: {:?})",
                    expected_count, timeout
                )
            });
    }

    async fn assert_logs_received(&self, expected_count: usize, timeout: Duration) {
        self.wait_for_logs(expected_count, timeout)
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "timed out waiting for {} logs (timeout: {:?})",
                    expected_count, timeout
                )
            });
    }

    async fn assert_span_exists(&self, span_name: &str, timeout: Duration) {
        let span_name = span_name.to_string();
        self.wait_until(
            |collector| collector.spans().iter().any(|s| s.span().name == span_name),
            timeout,
        )
        .await
        .unwrap_or_else(|_| {
            panic!(
                "span '{}' not found within timeout {:?}",
                span_name, timeout
            )
        });
    }

    async fn assert_span_not_exists(&self, span_name: &str, wait_duration: Duration) {
        tokio::time::sleep(wait_duration).await;

        let span_name = span_name.to_string();
        self.with_collector(|collector| {
            let exists = collector.spans().iter().any(|s| s.span().name == span_name);
            if exists {
                panic!("span '{}' was found but expected to not exist", span_name);
            }
        })
        .await;
    }

    async fn assert_span_has_attribute(
        &self,
        span_name: &str,
        attribute_key: &str,
        expected_value: &str,
        timeout: Duration,
    ) {
        self.assert_span_exists(span_name, timeout).await;

        let span_name = span_name.to_string();
        let attribute_key = attribute_key.to_string();
        let expected_value = expected_value.to_string();

        self.with_collector(|collector| {
            collector
                .expect_span_with_name(&span_name)
                .with_attribute(attribute_key.clone(), expected_value.clone())
                .assert_exists();
        })
        .await;
    }

    async fn span_count(&self) -> usize {
        let mut count = 0;
        self.with_collector(|collector| {
            count = collector.span_count();
        })
        .await;
        count
    }

    async fn metric_count(&self) -> usize {
        let mut count = 0;
        self.with_collector(|collector| {
            count = collector.metric_count();
        })
        .await;
        count
    }

    async fn log_count(&self) -> usize {
        let mut count = 0;
        self.with_collector(|collector| {
            count = collector.log_count();
        })
        .await;
        count
    }
}
