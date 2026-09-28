//! Arona 功能插件模板（配套文档：框架仓库根 `PLUGIN_DEVELOPMENT.md`）
//!
//! 形态与 mirai 的插件一致：本 crate 编译成 `my_plugin.dll`，用户在**启动前**把它放进
//! 框架运行目录的 `plugins/<插件 id>/`，框架启动时扫描、握手、装配。宿主不链接它，
//! 所以改插件代码不需要重编框架，换一次 dll 就生效。
//!
//! 与框架的全部关系就是文件末尾那一行 [`arona::export_arona_plugin!`]（导出四个 C 符号）
//! 加上下面这些生命周期回调。**插件不提供 `main`、也不能独立运行**，一切由框架驱动。
//!
//! 这里各演示一发：
//! - `install`：登记一个分群功能开关 + 一块强类型配置（`config/my-plugin/arona.yml`）
//! - `configure`：一条命令（自己回话与交框架回话两种写法里的一种）+ 一个进群欢迎钩子
//! - `start`：按配置排一个每日任务
//! - `stop`：什么都不写——回收本来就是框架的事（框架会掐掉本插件名下的任务与订阅）
//!
//! 更完整的接口（对象模型 Bot/Group/Member、出站改写钩子、消息段全谱、聊天记录翻页、
//! HTTP 交宿主代发）见框架仓库的 `plugins/hello/` 与文档 §9~§21。

mod config;

use std::sync::Arc;

use arona::onebot::EventContext;
use arona::onebot::hooks::{BodyFilter, HookFlow, NoticeKind, event_handler};
use arona::plugin::{AronaPlugin, PluginContext, PluginMeta, PluginRegistrar};
use arona::runtime::config::Feature;
use arona::runtime::dispatcher::{CommandRegistration, handler};
use arona::runtime::log;
use arona::runtime::message::{MessageTarget, OutgoingMessage};
use arona::runtime::priority::ListenerPriority;
use arona::runtime::services::send_message;

use crate::config::{MyPluginConfig, SECTION};

/// 功能开关的 key：命令的 `with_feature` 与钩子的 `listen_feature` 共用它。
/// 群管理页里关掉这一项时，命令与钩子会一起停止投递，不留"面板关了钩子还在收"的尾巴。
/// 必须是 `&'static str`：拿 `const` 定义最省事，`format!` 出来的 `String` 编不过。
const FEATURE: &str = "my-plugin";

/// 每日任务的登记名：同名任务会被直接替换，所以配置每热重载一次就照新值重排
const DAILY_JOB: &str = "MyPluginDailyGreeting";

#[derive(Default)]
pub struct MyPlugin;

impl AronaPlugin for MyPlugin {
    /// 只有 id、name、version 是规范强制的。**id 决定落盘目录与配置键，定了就别改**
    fn meta(&self) -> PluginMeta {
        PluginMeta::new(
            "my-plugin",
            "MyPlugin",
            env!("CARGO_PKG_VERSION"),
            "我的第一个 Arona 插件",
        )
        .with_author("你的名字")
    }

    /// 登记阶段：框架配置还没加载，此刻只能往外登记东西
    fn install(&self, reg: &PluginRegistrar) -> Result<(), String> {
        reg.feature(Feature {
            key: FEATURE,
            name: "我的功能",
            description: "问候与每日播报",
        });
        // 必须在这里登记配置区：框架据此生成 config/my-plugin/arona.yml 的带注释模板
        reg.config::<MyPluginConfig>(SECTION);
        Ok(())
    }

    /// 装配阶段：两份配置已就绪、OneBot 还没连上，命令与钩子都在这一步交出去
    fn configure(&self, ctx: &PluginContext) -> Result<(), String> {
        ctx.commands(vec![
            CommandRegistration::new(
                vec!["/问候".into(), "/greet".into()],
                "回一句问候",
                // handler 只要求闭包返回一个 future：`|_ctx, _args| async move { .. }` 就够，
                // 不必再包一层 Box::pin。返回 Some(消息) 由框架发回原会话（群聊回群、私聊回人）；
                // 自己已经 reply 过就返回 None，框架不会重复发。
                handler(|_context, _arguments| async move {
                    Some(OutgoingMessage::text("老师好（来自插件模板）"))
                }),
            )
            .with_feature(FEATURE)
            .with_usage("/问候"),
        ]);

        // 只订阅"成员进群"这一种子事件：子类判定由框架完成，插件不必自己比字符串。
        // 优先用 BodyFilter::notice(..) 这类构造器，手写枚举容易漏掉动作那一格。
        let greeting_ctx = ctx.clone();
        ctx.listen_where(
            &[BodyFilter::notice(NoticeKind::GroupIncrease, "")],
            ListenerPriority::default(),
            event_handler(move |context: Arc<EventContext>| {
                // 读配置要拿 ctx，而闭包得是 'static：先 clone 一份归属，再在体里取值
                let setup = greeting_ctx.clone();
                async move {
                    let greeting = setup.config::<MyPluginConfig>(SECTION).get().greeting;
                    // sender_name() 是 Option<&str>：实现端没给名片也没给昵称就是 None
                    let who = context.sender_name().unwrap_or("新同学");
                    // reply 回到事件来源会话（这里是那个群），走框架统一发送口，
                    // 所以出站改写钩子与门控都在路上
                    let _ = context.reply(format!("{greeting}，欢迎 {who} 进群")).await;
                    HookFlow::Pass
                }
            }),
        );
        Ok(())
    }

    /// 启动阶段：OneBot 已连上，后台任务与定时任务在这里拉起
    fn start(&self, ctx: &PluginContext) -> Result<(), String> {
        self.schedule_daily(ctx);
        Ok(())
    }

    /// 本插件的 config/<id>/arona.yml 被热重载后回调：按新值重排即可，不必自己记"上次是几点"
    fn on_config_reload(&self, ctx: &PluginContext) {
        self.schedule_daily(ctx);
    }
}

impl MyPlugin {
    /// 每日任务的写法：`JobFn` 是**同步**闭包，真要发消息/联网得把异步工作丢进本插件的任务作用域。
    /// 用 `ctx.spawn_as(..)` 而不是裸 `tokio::spawn`，否则插件被停用时这个任务收不回去。
    fn schedule_daily(&self, ctx: &PluginContext) {
        let Some(hour) = ctx.config::<MyPluginConfig>(SECTION).get().daily_hour else {
            return; // 没配时间就不排任务
        };
        let scheduled = ctx.clone();
        ctx.daily_job(
            hour.clamp(0, 23) as u32,
            DAILY_JOB,
            Arc::new(move || {
                let context = scheduled.clone();
                scheduled.spawn_as("每日问候", async move {
                    let config = context.config::<MyPluginConfig>(SECTION).get();
                    let Some(group_id) = config.daily_group else {
                        return;
                    };
                    // 主动发消息的统一出口：会过出站钩子，也受群授权与黑名单约束
                    let _ = send_message(
                        MessageTarget::Group(group_id),
                        OutgoingMessage::text(config.greeting),
                    )
                    .await;
                });
            }),
        );
        log::info(format!("我的插件：每日问候已排定，每天 {hour} 点"));
    }
}

// 框架装载本 dll 的入口：导出 ABI 号、工具链指纹、宿主上行通道、造实例四个 C 符号。
// 这里用普通注释而不是 ///：宏调用不产生可文档化的条目，rustdoc 会警告。
// 空的单位结构体直接写类型名（`MyPlugin::default()` 会被 clippy 念一句）；
// 一旦 MyPlugin 有了字段，就换回 `MyPlugin::default()` 或自己的 `MyPlugin::new(..)`。
// 插件永远不要写 `main`——可执行文件只来自宿主。
arona::export_arona_plugin!(MyPlugin);
