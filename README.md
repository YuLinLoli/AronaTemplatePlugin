# Arona 插件模板

一个能直接编译成 dll 的 Arona 功能插件骨架。改三处就能变成你自己的插件：
`Cargo.toml` 的 `name`/`[lib] name`、`src/lib.rs` 里的 `PluginMeta::new(..)` 与 `FEATURE`、
`src/config.rs` 里的 `SECTION` 和配置字段。

配套的完整规范在**框架仓库根目录的 `PLUGIN_DEVELOPMENT.md`**，本文只讲开工与不被拒载。

## 上手

```bash
# 1. 框架仓库与本模板同级放一份（Cargo.toml 里的 path 依赖指的就是 ../Arona-rs）
git clone https://github.com/YuLinLoli/Arona-rs ../Arona-rs

# 2. 编译，产物 target/release/my_plugin.dll
cargo build --release

# 3. 丢进框架运行目录，重启框架
cp target/release/my_plugin.dll "<框架运行目录>/plugins/my-plugin/"
```

第 3 步之后框架会扫描 `plugins/`（含它的一级子目录）、握手、登记，管理面板的「插件管理」页里
就能看到并开关。**插件不能独立运行**：没有 `main`，也不产出带插件的可执行文件，
可执行文件永远来自宿主 `arona-rs.exe`。

不想依赖本地 clone 就把 `Cargo.toml` 里那一行换成 crates.io（框架 1.1.0 起发布在这里）：

```toml
arona = { package = "arona-rs", version = "1.1.0", default-features = false }
```

注意包名是 **`arona-rs`**：crates.io 上的 `arona` 是另一个项目。库名仍是 `arona`，
所以源码里照旧 `use arona::...`。

## 四条硬口径，差一条就被拒载

dll 是被宿主进程直接映射进来的，`arona` 静态链接成了第二份，所以框架在握手时逐项核对：

| 核对项 | 本模板怎么保证 |
| --- | --- |
| rustc 版本 | `rust-toolchain.toml` 钉 `1.98.0`，与框架同一行；框架升级工具链时插件要跟着改再重编 |
| CRT 链接方式 | `.cargo/config.toml` 里那两段 `crt-static`，**必须与框架仓库那份逐字一致**，不要加别的 rustflags |
| 框架版本 + feature 集合 | `default-features = false`（别把 `gui` 拖进 dll）；arona 的版本与目标宿主完全一致 |
| 符号布局号 | 框架的 `ABI_LAYOUT` 变了要重编插件；当前是 `5`，见文档 §22 |

不符合时框架**不会静默忽略**，而是写一条中文原因的拒载日志。所以先看 `logs/`，再怀疑自己代码。
本地跑宿主调试时被提权逻辑拦住，加环境变量 `ARONA_NO_ELEVATE=1`。

## 日常调试

```bash
cargo test                    # rlib 那份给单测用，纯逻辑不必起宿主
cargo build --release         # 改一行代码 = 重编 + 重拷 dll + 重启框架
```

要下断点就在 IDE 里 **attach 到 `arona-rs.exe`**，能直接停在插件源码上；
若要在发布构建里调试，取消 `Cargo.toml` 里 `[profile.release] debug = 1` 的注释再重编 dll。

## 模板里演示了什么

| 位置 | 内容 | 对应文档 |
| --- | --- | --- |
| `install` | 登记分群功能开关 + 一块强类型配置（框架据此生成带注释的 `config/my-plugin/arona.yml`） | §8、§12 |
| `configure` | 一条命令（返回 `Some(消息)` 交框架发回原会话）、一个只收"成员进群"的钩子 | §8、§9 |
| `start` / `on_config_reload` | 按配置排每日任务；`JobFn` 是同步闭包，异步工作丢进 `ctx.spawn_as` 才收得回 | §10 |
| 末尾一行宏 | `arona::export_arona_plugin!` 导出四个 C 符号，框架靠它握手 | §15、§22 |

`Feature` 的 `key`、配置区键名、`with_feature` / `listen_feature` 那几处都是 `&'static str`：
字符串字面量与 `const` 满足，`format!` 出来的 `String` 编不过（文档 §8 有写法）。

## 发布插件时必须写明的三件事

```
适用于 Arona-rs 1.1.0（框架契约 1.0.0，符号布局 5）
构建工具链：rustc 1.98.0，x86_64-pc-windows-msvc，--release，crt-static
下载后放进 <框架运行目录>/plugins/<你的id>/，重启框架
```

版本与布局号去框架仓库的 Release 页与 `PLUGIN_DEVELOPMENT.md` §22 核对，别照抄本文件——
框架抬版本或抬布局之后，这里不改就会误导用户。
