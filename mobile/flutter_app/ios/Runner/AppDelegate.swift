import Flutter
import UIKit

/// 包装插件注册逻辑，符合 FlutterPluginRegistrant 协议。
/// FlutterViewController 从 storyboard 隐式创建 engine 后会调用 register(with:)。
final class PluginRegistrant: NSObject, FlutterPluginRegistrant {
  func register(with registry: FlutterPluginRegistry) {
    GeneratedPluginRegistrant.register(with: registry)
    SoundLinkPlugin.register(
      with: registry.registrar(forPlugin: "SoundLinkPlugin")!)
  }
}

@main
@objc class AppDelegate: FlutterAppDelegate {
  override init() {
    super.init()
    // 必须在 storyboard 加载前（即 init 阶段）设置，而非 didFinishLaunchingWithOptions。
    //
    // iOS 启动顺序：UIApplicationMain → AppDelegate.init → willFinishLaunching
    // → 加载 Main.storyboard → FlutterViewController.awakeFromNib
    //   → 隐式 FlutterEngine 创建 → 检查 appDelegate.pluginRegistrant
    //   → [registrant registerWithRegistry:self]
    // → didFinishLaunchingWithOptions → viewDidLoad → launchEngine → Dart isolate
    //
    // 若在 didFinishLaunchingWithOptions 中赋值，storyboard 加载时 pluginRegistrant
    // 仍为 nil，FlutterAppDelegate getter 返回 nil，[nil registerWithRegistry:] 是
    // ObjC no-op，插件未注册。Release（AOT）下 Dart isolate 启动极快，立即调用
    // SharedPreferences.getInstance() → channel 无 handler → channel-error。
    // Debug（JIT）因 VM Service 启动慢而"碰巧"未触发。
    self.pluginRegistrant = PluginRegistrant()
  }

  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }
}
