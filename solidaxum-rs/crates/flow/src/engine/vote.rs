use crate::enums::CooperateType;

/// 判断多人审批是否达到推进阈值
///
/// - `node_ratio`: 节点配置的比例字符串（可含 @@sequence 后缀）
/// - `total`: 初始总用户数（remaining + processed）
/// - `processed`: 已 PASS 的用户数（含当前这次）
///
/// 新增 passCount=N 固定票数策略。
pub fn should_advance(
    node_ratio: &str,
    total: usize,
    processed: usize,
) -> bool {
    let ratio = CooperateType::strip_sequence(node_ratio);
    if let Some(n) = ratio.strip_prefix("passCount=") {
        return n.parse::<usize>().map(|c| processed >= c).unwrap_or(false);
    }
    if CooperateType::is_or_sign(ratio) {
        processed >= 1
    } else if CooperateType::is_countersign(ratio) {
        processed >= total
    } else if let Ok(r) = ratio.parse::<f64>() {
        let needed = (total as f64 * r / 100.0).ceil() as usize;
        processed >= needed
    } else {
        processed >= total
    }
}

/// 驳回阈值判定：rejectCount=N 固定票数；否则按 (100 - nodeRatio) 比例。
pub fn reject_advance(node_ratio: &str, total: usize, rejected: usize) -> bool {
    let ratio = CooperateType::strip_sequence(node_ratio);
    if let Some(n) = ratio.strip_prefix("rejectCount=") {
        return n.parse::<usize>().map(|c| rejected >= c).unwrap_or(false);
    }
    if CooperateType::is_or_sign(ratio) {
        rejected >= 1 // 或签：一人驳回即驳回
    } else if let Ok(r) = ratio.parse::<f64>() {
        let reject_ratio = (100.0 - r).max(0.0);
        rejected >= (total as f64 * reject_ratio / 100.0).ceil() as usize
    } else {
        rejected >= 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn or_sign_any_one_passes() {
        assert!(should_advance("0", 3, 1));
        assert!(should_advance("", 3, 1));
        assert!(!should_advance("", 3, 0));
    }

    #[test]
    fn countersign_requires_all() {
        assert!(!should_advance("100", 3, 2));
        assert!(should_advance("100", 3, 3));
        assert!(should_advance("100@@sequence", 3, 3));
    }

    #[test]
    fn sequence_suffix_stripped() {
        // 100@@sequence = countersign
        assert!(!should_advance("100@@sequence", 3, 2));
        assert!(should_advance("100@@sequence", 3, 3));
    }

    #[test]
    fn pass_count_fixed_threshold() {
        // passCount=2 -> 满 2 票通过（与比例无关）
        assert!(should_advance("passCount=2", 5, 2));
        assert!(!should_advance("passCount=2", 5, 1));
    }

    #[test]
    fn reject_ratio_uses_complement() {
        // node_ratio=60 -> 通过需 60%，驳回需 (100-60)=40%
        // total=5, rejected=2 -> 40% of 5 = 2 -> 达到驳回阈值
        assert!(reject_advance("60", 5, 2));
        assert!(!reject_advance("60", 5, 1));
    }

    #[test]
    fn reject_count_fixed_threshold() {
        assert!(reject_advance("rejectCount=2", 5, 2));
        assert!(!reject_advance("rejectCount=2", 5, 1));
    }
}
