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
  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    // Flutter 3.44：FlutterViewController 从 storyboard 加载时会隐式创建 FlutterEngine，
    // 并在引擎就绪后调用 pluginRegistrant.register(with:) 注册插件。
    // 这是 storyboard 模式下官方推荐的插件注册方式（见 FlutterAppDelegate.h）。
    self.pluginRegistrant = PluginRegistrant()
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }
}
