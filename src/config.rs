//! 本插件自己那块配置，落到框架运行目录的 `config/<插件 id>/arona.yml`。
//!
//! 插件只写这个 serde 结构体 + 字段的注释：带注释的 YAML 模板生成、加载、热重载、
//! 未知子键过滤、旧配置搬迁全部由框架完成（PLUGIN_DEVELOPMENT.md §12）。
//! 读用 `ctx.config::<MyPluginConfig>(SECTION).get()`，写用 `ConfigEntry::update(..)`。

use arona::config::arona::PluginConfig;
use serde::{Deserialize, Serialize};

/// 配置区键名。惯例是与插件 id 一致。
pub const SECTION: &str = "my-plugin";

/// `#[serde(default)]` 让"用户只写了一项"也能读出来：以后新增字段不会让老配置失效
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MyPluginConfig {
    /// 回复里用的那句问候语
    pub greeting: String,
    /// 每天几点主动发一次（0-23）；留空表示不主动发
    pub daily_hour: Option<i64>,
    /// 主动发给哪个群；与 daily_hour 同时配上才会发
    pub daily_group: Option<i64>,
}

impl Default for MyPluginConfig {
    fn default() -> Self {
        MyPluginConfig {
            greeting: "老师好".to_string(),
            daily_hour: None,
            daily_group: None,
        }
    }
}

impl PluginConfig for MyPluginConfig {
    const TITLE: &'static str = "我的插件";
    const DOC: &'static str = "示例配置区。改完保存即热生效，不必重启机器人。";

    /// 这里是注释的唯一来源：框架生成 config/<id>/arona.yml 时按路径逐字段取
    fn comment(path: &str) -> Option<&'static str> {
        Some(match path {
            "greeting" => "回复里用的那句问候语",
            "daily_hour" => "每天几点主动发一次(0-23)，留空表示不主动发",
            "daily_group" => "主动发给哪个群，与 daily_hour 同时配上才会发",
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 默认值要保证"什么都不配"时插件不会主动骚扰人
    #[test]
    fn defaults_stay_quiet() {
        let config = MyPluginConfig::default();
        assert_eq!(
            config.daily_hour, None,
            "默认不主动发任何消息：主动推送得由用户自己点名"
        );
        assert_eq!(config.daily_group, None);
        assert!(!config.greeting.is_empty());
    }
}
