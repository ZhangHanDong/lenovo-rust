// 第 8 课 · headless 验证：在宿主（macOS）上经 flutter_rust_bridge 生成的 Dart 绑定，
// 真实调用 Rust（rust/src/api/simple.rs）并断言结果。
//
// 运行：
//   cd frb_demo
//   (cd rust && cargo build)                       # 先产出 librust_lib_frb_demo.dylib
//   flutter test test/rust_call_test.dart          # headless，无需模拟器/真机
//
// 它把 Rust 端的 .dylib 通过 ExternalLibrary 显式加载，故不依赖 cargokit 的运行时构建，
// 适合在 CI / 命令行做"frb 工程真实跑通"的回归验证。

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:frb_demo/src/rust/api/simple.dart';
import 'package:frb_demo/src/rust/frb_generated.dart';

void main() {
  setUpAll(() async {
    await RustLib.init(
      externalLibrary:
          ExternalLibrary.open('rust/target/debug/librust_lib_frb_demo.dylib'),
    );
  });

  test('greet：同步 #[frb(sync)] 直接返回拼接字符串', () {
    expect(greet(name: 'Lenovo'), 'Hello, Lenovo!');
  });

  test('add：默认异步，在 Rust 侧求和后 await 取回', () async {
    expect(await add(a: 2, b: 40), 42);
  });

  test('divide：正常情况返回商', () async {
    expect(await divide(a: 9, b: 3), closeTo(3.0, 1e-9));
  });

  test('divide：除零 → Rust 的 Err 在 Dart 侧抛成异常', () async {
    await expectLater(divide(a: 1, b: 0), throwsA(anything));
  });
}
