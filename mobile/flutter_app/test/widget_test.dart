import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:soundlink/app.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('SoundLink app renders home navigation', (
    WidgetTester tester,
  ) async {
    SharedPreferences.setMockInitialValues({});

    await tester.pumpWidget(const SoundLinkApp());
    await tester.pump();

    expect(find.text('状态：未连接'), findsOneWidget);
    expect(find.byType(NavigationBar), findsOneWidget);
    expect(find.text('设备'), findsWidgets);
    expect(find.text('设置'), findsOneWidget);
  });
}
