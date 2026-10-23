#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Start = 0,
    Between = 1,
    End = 2,
    Serial = 3,
    Parallel = 4,
    Inclusive = 5,
}

impl NodeType {
    pub fn from_i8(value: i8) -> Option<Self> {
        match value {
            0 => Some(Self::Start),
            1 => Some(Self::Between),
            2 => Some(Self::End),
            3 => Some(Self::Serial),
            4 => Some(Self::Parallel),
            5 => Some(Self::Inclusive),
            _ => None,
        }
    }

    pub fn is_start(&self) -> bool {
        matches!(self, Self::Start)
    }

    pub fn is_end(&self) -> bool {
        matches!(self, Self::End)
    }

    pub fn is_gate_way(&self) -> bool {
        matches!(self, Self::Serial | Self::Parallel | Self::Inclusive)
    }

    pub fn is_gate_way_serial(&self) -> bool {
        matches!(self, Self::Serial)
    }

    pub fn is_gate_way_parallel(&self) -> bool {
        matches!(self, Self::Parallel)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipType {
    Pass,
    Reject,
    None,
}

impl SkipType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Reject => "REJECT",
            Self::None => "NONE",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "PASS" => Some(Self::Pass),
            "REJECT" => Some(Self::Reject),
            "NONE" => Some(Self::None),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowStatus {
    ToBeSubmit = 0,
    Approval = 1,
    Pass = 2,
    AutoPass = 3,
    Terminate = 4,
    Nullify = 5,
    Cancel = 6,
    Retrieve = 7,
    Finished = 8,
    Reject = 9,
    Invalid = 10,
    TaskBack = 11,
    ReStart = 12,
    Pending = 13,
}

impl FlowStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ToBeSubmit => "0",
            Self::Approval => "1",
            Self::Pass => "2",
            Self::AutoPass => "3",
            Self::Terminate => "4",
            Self::Nullify => "5",
            Self::Cancel => "6",
            Self::Retrieve => "7",
            Self::Finished => "8",
            Self::Reject => "9",
            Self::Invalid => "10",
            Self::TaskBack => "11",
            Self::ReStart => "12",
            Self::Pending => "13",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "0" => Some(Self::ToBeSubmit),
            "1" => Some(Self::Approval),
            "2" => Some(Self::Pass),
            "3" => Some(Self::AutoPass),
            "4" => Some(Self::Terminate),
            "5" => Some(Self::Nullify),
            "6" => Some(Self::Cancel),
            "7" => Some(Self::Retrieve),
            "8" => Some(Self::Finished),
            "9" => Some(Self::Reject),
            "10" => Some(Self::Invalid),
            "11" => Some(Self::TaskBack),
            "12" => Some(Self::ReStart),
            "13" => Some(Self::Pending),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CooperateType {
    Approval = 1,
    Transfer = 2,
    Depute = 3,
    Countersign = 4,
    Vote = 5,
    AddSignature = 6,
    ReductionSignature = 7,
    /// 协同：发起人退出,协同人编组会签,全过即推进(无 Java 对位,
    /// 引擎原语,对齐参考实现 needCoor 业务模式)。
    Collaborate = 8,
}

impl CooperateType {
    pub fn from_i8(value: i8) -> Option<Self> {
        match value {
            1 => Some(Self::Approval),
            2 => Some(Self::Transfer),
            3 => Some(Self::Depute),
            4 => Some(Self::Countersign),
            5 => Some(Self::Vote),
            6 => Some(Self::AddSignature),
            7 => Some(Self::ReductionSignature),
            8 => Some(Self::Collaborate),
            _ => None,
        }
    }

    /// 协作方式名称(对齐 warm-flow `CooperateType.getValueByKey`)。
    pub fn label(value: i8) -> Option<&'static str> {
        Some(match Self::from_i8(value)? {
            Self::Approval => "审批",
            Self::Transfer => "转办",
            Self::Depute => "委派",
            Self::Countersign => "会签",
            Self::Vote => "票签",
            Self::AddSignature => "加签",
            Self::ReductionSignature => "减签",
            Self::Collaborate => "协同",
        })
    }

    /// or-sign: ratio == 0
    pub fn is_or_sign(ratio: &str) -> bool {
        ratio.is_empty() || ratio == "0"
    }

    /// countersign: ratio == 100
    pub fn is_countersign(ratio: &str) -> bool {
        ratio == "100"
    }

    /// vote-sign: 0 < ratio < 100
    pub fn is_vote_sign_ratio(ratio: &str) -> bool {
        if let Ok(r) = ratio.parse::<f64>() {
            r > 0.0 && r < 100.0
        } else {
            false
        }
    }

    /// 去掉后缀 @@sequence
    pub fn strip_sequence(ratio: &str) -> &str {
        ratio.strip_suffix("@@sequence").unwrap_or(ratio)
    }

    /// 是否有 sequential 标记
    pub fn is_sequence(ratio: &str) -> bool {
        ratio.ends_with("@@sequence")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityStatus {
    Suspended = 0,
    Active = 1,
}

impl ActivityStatus {
    pub fn from_i8(value: i8) -> Option<Self> {
        match value {
            0 => Some(Self::Suspended),
            1 => Some(Self::Active),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishStatus {
    Unpublished = 0,
    Published = 1,
    Expired = 9,
}

impl PublishStatus {
    pub fn from_i8(value: i8) -> Option<Self> {
        match value {
            0 => Some(Self::Unpublished),
            1 => Some(Self::Published),
            9 => Some(Self::Expired),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserType {
    Approval = 1,
    Transfer = 2,
    Depute = 3,
    /// 抄送人权限，对应 Java TaskAssigneeType.COPY("4")。
    /// 抄送数据复用 flow_user 表，仅以 type=4 区分，不另立 flow_copy 表。
    Copy = 4,
    /// 协同人：collaborate 操作生成的办理人(无 Java 对位)。
    /// 同任务的协同编组触发 try_halt 的会签语义。
    Collaborate = 5,
}

impl UserType {
    pub fn as_i8(self) -> i8 {
        self as i8
    }

    pub fn from_i8(value: i8) -> Option<Self> {
        match value {
            1 => Some(Self::Approval),
            2 => Some(Self::Transfer),
            3 => Some(Self::Depute),
            4 => Some(Self::Copy),
            5 => Some(Self::Collaborate),
            _ => None,
        }
    }
}
