//! RuoYi 语义状态列枚举 —— 收敛散落调用点的 "0"/"1" 魔法串。
//!
//! Java 基线(RuoYi-Vue-Plus):status 0=正常 1=停用;del_flag 0=存在
//! 1=删除(`SysUser.java:86` 字段注释 + `ry_vue.sql` schema 注释,
//! MyBatis-Plus `@TableLogic` 默认值同);menu.visible 0=显示 1=隐藏。
//!
//! 本阶段只替换**调用点字面量**,实体/Bo 字段类型保持 String —— 零契约
//! 变更、零类型涟漪;serde 以字符串往返,为后续字段类型化预留。

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

macro_rules! str_backed_enum {
    ($(#[$meta:meta])* $name:ident {
        $($(#[$vmeta:meta])* $variant:ident = $val:literal),+ $(,)?
    }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name {
            $($(#[$vmeta])* #[serde(rename = $val)] $variant,)+
        }

        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $val,)+
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($val => Ok($name::$variant),)+
                    _ => Err(format!(
                        "{} 未知取值: {s}", stringify!($name))),
                }
            }
        }
    };
}

str_backed_enum! {
    /// 通用状态(sys_user/sys_dept/sys_role/sys_post/... 的 status 列)。
    Status {
        /// 0 = 正常
        Normal = "0",
        /// 1 = 停用
        Disabled = "1",
    }
}

str_backed_enum! {
    /// 软删标志(del_flag 列,语义对齐 MyBatis-Plus @TableLogic)。
    DelFlag {
        /// 0 = 存在
        Alive = "0",
        /// 1 = 删除
        Deleted = "1",
    }
}

str_backed_enum! {
    /// 菜单可见性(sys_menu.visible 列)。
    Visible {
        /// 0 = 显示
        Show = "0",
        /// 1 = 隐藏
        Hide = "1",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_roundtrip() {
        assert_eq!(Status::Normal.as_str(), "0");
        assert_eq!(Status::Disabled.to_string(), "1");
        assert_eq!("1".parse::<DelFlag>().unwrap(), DelFlag::Deleted);
        assert!("2".parse::<Visible>().is_err());
        assert_eq!(serde_json::to_string(&Status::Disabled).unwrap(), "\"1\"");
    }
}
