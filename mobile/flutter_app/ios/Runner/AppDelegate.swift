import Flutter
import UIKit

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate {
  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    // Flutter 3.44 起，FlutterAppDelegate 使用 implicit engine；
    // window/rootViewController 在 super 调用后才会就绪，
    // 但插件注册统一放到 didInitializeImplicitFlutterEngine（更可靠的引擎就绪时机）。
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  func didInitializeImplicitFlutterEngine(_ engineBridge: FlutterImplicitEngineBridge) {
    // 1) 生成式插件（shared_preferences 等）。
    GeneratedPluginRegistrant.register(with: engineBridge.pluginRegistry)
    // 2) 手写插件：SoundLinkPlugin（com.soundlink/platform 通道）。
    //    之前在 application:didFinishLaunchingWithOptions: 中通过
    //    window?.rootViewController as? FlutterViewController 注册，
    //    但新版 Flutter 下 rootViewController 不一定是 FlutterViewController，
    //    导致注册被静默跳过 → MissingPluginException。
    SoundLinkPlugin.register(
      with: engineBridge.pluginRegistry.registrar(forPlugin: "SoundLinkPlugin")!
    )
  }
}
