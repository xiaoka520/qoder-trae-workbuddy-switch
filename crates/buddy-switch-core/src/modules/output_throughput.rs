use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 配对累计可用样本的输出 Token 与端到端耗时。
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OutputThroughput {
    output_tokens: u64,
    duration_ms: u64,
}

impl OutputThroughput {
    pub(super) fn from_log(value: &Value, duration_field: &str) -> Self {
        match (
            value.get("status").and_then(Value::as_u64),
            value.get("completionTokens").and_then(Value::as_u64),
            value.get(duration_field).and_then(Value::as_u64),
        ) {
            (Some(200..=299), Some(output_tokens @ 1..), Some(duration_ms @ 1..)) => Self {
                output_tokens,
                duration_ms,
            },
            _ => Self::default(),
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.duration_ms == 0
    }

    pub(super) fn observed(self) -> Option<Self> {
        (!self.is_empty()).then_some(self)
    }

    pub(super) fn add(&mut self, other: Self) {
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.duration_ms = self.duration_ms.saturating_add(other.duration_ms);
    }

    /// 校正轮的快照可能重叠。保留累计耗时更长的一对读数。
    pub(super) fn keep_larger_snapshot(&mut self, other: Self) {
        if other.duration_ms > self.duration_ms {
            *self = other;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn includes_only_success_with_output_and_full_duration() {
        let sample = |status, output, duration, field| {
            OutputThroughput::from_log(
                &json!({ "status": status, "completionTokens": output, "latencyMs": 250, "totalMs": duration }),
                field,
            )
        };
        let valid = sample(200, 60, 1_000, "totalMs");
        assert_eq!(
            json!(valid.observed()),
            json!({ "outputTokens": 60, "durationMs": 1_000 })
        );
        assert_eq!(sample(502, 60, 1_000, "totalMs").observed(), None);
        assert_eq!(sample(200, 0, 1_000, "totalMs").observed(), None);
        assert_eq!(sample(200, 60, 0, "totalMs").observed(), None);
        assert_eq!(sample(200, 60, 0, "latencyMs").duration_ms, 250);
    }
}
