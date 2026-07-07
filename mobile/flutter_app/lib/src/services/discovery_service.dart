// 设备发现服务：mDNS / Bonjour / NSD 查询 `_soundlink._udp.local.`。
//
// 使用纯 Dart 的 multicast_dns 包，iOS/Android 双端复用。
// 解析 PTR/SRV/TXT 得到桌面 Receiver 列表。
//
// 注：multicast_dns 将多条 TXT 字符串以换行拼接为单个 text，这里按行拆分再解析 key=value。
//
// Android 不支持 SO_REUSEPORT（dart:io 会打 "reusePort not supported on this platform"
// 错误日志），此处注入自定义 socketFactory，关掉 reusePort（reuseAddress 已足够）。

import 'dart:async';
import 'dart:io';

import 'package:multicast_dns/multicast_dns.dart';

import '../constants.dart' as k;
import '../models/device.dart';

class DiscoveryService {
  final String serviceType;
  bool _running = false;

  DiscoveryService({this.serviceType = k.mdnsServiceType});

  /// 创建一个不使用 reusePort 的 MDnsClient（兼容 Android）。
  MDnsClient _createClient() {
    return MDnsClient(
      rawDatagramSocketFactory:
          (
            dynamic host,
            int port, {
            bool reuseAddress = true,
            bool reusePort = false,
            int ttl = 0,
          }) {
            // 强制 reusePort=false：Android 不支持 SO_REUSEPORT，dart:io 会报错。
            // reuseAddress=true 已足以让多个查询 socket 共存。
            return RawDatagramSocket.bind(
              host,
              port,
              reuseAddress: true,
              reusePort: false,
              ttl: ttl,
            );
          },
    );
  }

  /// 启动一次扫描（持续约 4s），返回发现的设备列表。
  Future<List<DiscoveredDevice>> scan({
    Duration duration = const Duration(seconds: 4),
  }) async {
    final client = _createClient();
    await client.start();
    _running = true;

    // SRV.name -> {target, port}
    final srvRecords = <String, _Srv>{};
    // SRV.name -> TXT map
    final txtRecords = <String, Map<String, String>>{};

    try {
      // 限时收集。
      final timer = Future<void>.delayed(duration).then((_) {
        _running = false;
      });

      await for (final ptr in client.lookup<PtrResourceRecord>(
        ResourceRecordQuery.serverPointer(serviceType),
      )) {
        if (!_running) break;
        final instanceName = ptr.domainName;

        // 查 SRV。
        await for (final srv in client.lookup<SrvResourceRecord>(
          ResourceRecordQuery.service(instanceName),
        )) {
          srvRecords[srv.name] = _Srv(srv.target, srv.port);
        }
        // 查 TXT。
        await for (final txt in client.lookup<TxtResourceRecord>(
          ResourceRecordQuery.text(instanceName),
        )) {
          txtRecords[txt.name] = _parseTxt(txt.text);
        }
        if (!_running) break;
      }
      await timer;
    } finally {
      _running = false;
      client.stop();
    }

    final devices = <DiscoveredDevice>[];
    for (final entry in srvRecords.entries) {
      final srv = entry.value;
      final txt = txtRecords[entry.key] ?? {};
      var host = srv.target;
      if (host.endsWith('.')) host = host.substring(0, host.length - 1);
      // 尝试解析 A 记录得到 IP。
      var ip = host;
      try {
        final aClient = _createClient();
        await aClient.start();
        await for (final a in aClient.lookup<IPAddressResourceRecord>(
          ResourceRecordQuery.addressIPv4(srv.target),
        )) {
          ip = a.address.address;
          break;
        }
        aClient.stop();
      } catch (_) {
        // 使用主机名兜底。
      }
      devices.add(DiscoveredDevice.fromTxt(ip, txt));
    }
    return devices;
  }

  void stop() {
    _running = false;
  }

  /// 解析 multicast_dns 的 TXT text（多条以换行拼接）。
  Map<String, String> _parseTxt(String text) {
    final map = <String, String>{};
    for (final line in text.split(RegExp(r'\r?\n'))) {
      final trimmed = line.trim();
      if (trimmed.isEmpty) continue;
      final i = trimmed.indexOf('=');
      if (i > 0) {
        map[trimmed.substring(0, i)] = trimmed.substring(i + 1);
      }
    }
    return map;
  }
}

class _Srv {
  final String target;
  final int port;
  _Srv(this.target, this.port);
}
