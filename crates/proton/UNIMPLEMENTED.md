# Proton ライブラリの未実装一覧

`crates/proton` が現時点で **実装していない** 範囲と、その理由をまとめる。実装済みの範囲は各クレートの `src/lib.rs` と `.rikka/memory/proton-*.md` を参照。

プロトコル事実は `reference/` の Proton 公式クライアントから写し、コードは逐語コピーしない。

## 対象外にしたもの

Android 固有で Linux に対応物がないため、実装しない。

- プッシュ通知（FCM）。更新は `rikka_proton_client::EventLoop` のポーリングで受け取る。
- 支払い（Google Play 課金）、プロモ、テレメトリ、NPS、バグ報告。

## 外部要因で実装できないもの

- **VPN の WireGuard over TCP/TLS（Stealth）本体。** Proton の転送実装 `pvpnclient` は非公開のレジストリにしかない。`rikka-proton-vpn` は公式 Linux クライアントと同じく、Proton の NetworkManager プラグイン `org.freedesktop.NetworkManager.protun` が入っていればそれに設定を渡す。入っていなければ `Error::UncarriedProtocol`。
- **forwarding 鍵の生成。** proton-rpgp も rpgp 0.19 も、forwarding 鍵での復号はできるが生成はできない。グループ共有の Pass Vault の読み取りは実装済みだが、生成した forwarding 鍵による通しテストはない（ライブラリのテストデータで復号だけを確認している）。

## 実アカウント・実機で未確認

すべてのテストはローカルのダミー API で走り、ネットワークに出ない。次は本番 API・実機で確かめていない。

- 本番 API で往復を確認済みなのは、サインイン、鍵の解錠、Pass の Vault 一覧、Mail のラベルとメッセージ一覧、Drive のボリューム一覧、VPN のティアと接続元、Authenticator の鍵、セッション失効まで。書き込み系と、今回追加した機能はすべて未確認。
- リファレンスに応答例がなく、寛容に読んでいるフィールドがある。
  - Mail：`filters/check` の応答、Sieve の細部、`messages/import`。
  - Calendar：メンバー追加時の `Invitations`、全件一覧の `BeginID`、アラームのページング。
  - Drive：下書きリビジョンの GET が送信済みブロックを返すか。
  - Pass：`ItemKeyRotation` の型。
  - Account：`TOTPSharedSecret`。
- FIDO2：実際のセキュリティキーでの動作は未確認（`rikka-test-fido` の擬似キーで検証）。Proton がネイティブアプリの origin として `https://account.<rpId>` を受け付けるかも未確認。
- VPN：実 NetworkManager と実サーバーでの接続は未確認（接続制御は擬似 `TunnelControl`、local agent はローカルの相互 TLS サーバーで検証）。
- Drive 同期：実アカウントでの実行は未確認。overlayfs で xino が無いと、inode の重複で同一性判定を誤る可能性がある。

## 運用上の注意

- Key Transparency が使う CT ログ一覧は `rikka-certificate-transparency` が持つ。内蔵するのは Chrome の一覧署名鍵だけで、一覧は実行時に Chrome の v3 一覧（gstatic）を取得・署名検証してキャッシュする。一覧が未取得・期限切れ（70 日超）なら使う前に取得し、取れなければ `Error::LogList(LogListError::NotFetched | Expired)` で KT を無効にする。gstatic に届かない環境では KT は動かない。ルート証明書（`rikka-proton-key-transparency/data/`）に日付の期限はない。
- Key Transparency はサーバー時刻とローカル時刻のずれで自動無効化しない。どの時計を使うかは呼び出し側が `Pgp` の `Clock` で決める。
- `apps/system/proton` は 1 アカウントで、ファサードの `Accounts` は使っていない。
