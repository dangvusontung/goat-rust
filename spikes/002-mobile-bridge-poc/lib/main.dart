import 'package:flutter/material.dart';
import 'src/rust/api.dart';
import 'src/rust/frb_generated.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'goat-bridge POC',
      theme: ThemeData(colorScheme: ColorScheme.fromSeed(seedColor: Colors.green)),
      home: const BridgeCheckPage(),
    );
  }
}

class BridgeCheckPage extends StatefulWidget {
  const BridgeCheckPage({super.key});

  @override
  State<BridgeCheckPage> createState() => _BridgeCheckPageState();
}

class _BridgeCheckPageState extends State<BridgeCheckPage> {
  String _status = 'calling Rust...';
  List<ClubDto> _clubs = [];

  @override
  void initState() {
    super.initState();
    _callBridge();
  }

  Future<void> _callBridge() async {
    try {
      final hasGame = await hasActiveGame();
      final clubs = await listClubs();
      setState(() {
        _status = 'hasActiveGame() = $hasGame';
        _clubs = clubs;
      });
    } catch (e) {
      setState(() => _status = 'BRIDGE CALL FAILED: $e');
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('goat-bridge mobile POC')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(_status, style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 12),
            Text('listClubs() -> ${_clubs.length} clubs from Rust:'),
            Expanded(
              child: ListView.builder(
                itemCount: _clubs.length,
                itemBuilder: (context, i) => ListTile(
                  title: Text(_clubs[i].name),
                  subtitle: Text('${_clubs[i].divName} · strength ${_clubs[i].strength}'),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
