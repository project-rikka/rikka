# Rikka のインストール

Rikka は x86_64 マシン向けの NixOS 用デスクトップ環境です。ビルド済みのパッケージと
NixOS モジュールを flake `github:project-rikka/packages` で配布しているため、
NixOS の設定に組み込んでインストールします。

インストール方法は 2 つあります。

- [NixOS と Rikka をまとめて新規インストール](#nixos-と-rikka-をまとめて新規インストール)
  インストーラーがディスクのパーティション分割と初期設定を自動で行います。
- [既存の NixOS に Rikka を追加](#既存の-nixos-に-rikka-を追加)
  現在の設定を残したまま Rikka を導入します。

## 動作要件

- x86_64 マシン
- Vulkan 1.3 とカーネルモードセッティング (KMS) ドライバに対応した GPU（Intel、
  AMD、または NVIDIA Turing [GTX 16xx / RTX 20xx] 以降）。ほとんどの仮想マシンでは、
  仮想 GPU がコンポジターの要件を満たせないためデスクトップが動作しません。
- NixOS unstable（推奨）または最新の安定版
- flake ベースの設定（まだ channels を使っている場合は、下記の手順 1 で flake に移行します）

## NixOS と Rikka をまとめて新規インストール

1. マシンを UEFI モードで [NixOS のライブ USB](https://nixos.org/download/#nixos-iso)
   から起動し、ネットワークに接続します（Wi-Fi の場合は `nmtui` を使用します）。
2. 次のコマンドを実行します。

   ```sh
   curl -fsSL https://install.rikka.sh | sudo sh
   ```

3. 画面の指示に従って入力します。インストーラーが選択したディスクを消去し、
   `/etc/nixos` を生成して Rikka を組み込んだ NixOS をインストールします。
   完了したら再起動します。

インストーラーはディスクを消去するため、内蔵ディスクから起動している既存のシステム上では
スクリプトが自動的に停止します。すでに NixOS をインストール済みの場合は、次のセクションの
手順に従います。

## 既存の NixOS に Rikka を追加

設定には次の 4 つが必要です。

1. flake の input `rikka`
2. NixOS モジュール `rikka.nixosModules.default`
3. 設定ファイルから import する `rikka/` ディレクトリ（Rikka の設定アプリと Packages
   アプリが選択内容をここに書き込みます）
4. `rikka.enable = true;`

以下の手順は flake が `/etc/nixos` にある前提です。別の場所にある場合は
[/etc/nixos 以外にある設定](#etcnixos-以外にある設定) を参照します。

### 1. flake の使用

`/etc/nixos/flake.nix` がすでにある場合は、この手順をスキップします。

flake が無効な環境では、手順 5 のリビルドで有効化されるまで、すべての `nix` コマンドに
`--extra-experimental-features 'nix-command flakes'` を付けて実行します。

```sh
cd /etc/nixos
sudo nix --extra-experimental-features 'nix-command flakes' \
  flake init -t github:project-rikka/packages
```

`configuration.nix` と同じ階層に `flake.nix` と `rikka/` が作成されます。これで
手順 2 と 3 の内容も完了するため、手順 4 に進みます。

生成された `flake.nix` は `./configuration.nix` を Rikka 付きでビルドし、
`networking.hostName` をシステム名として使用します。`hardware-configuration.nix` 内で
`nixpkgs.hostPlatform` が設定されている必要がありますが、通常は `nixos-generate-config`
によって自動生成されます。記述がない場合は `nixpkgs.hostPlatform = "x86_64-linux";` を
追加します。flake は `NIX_PATH` の channels を参照できないため、`<nixpkgs/…>` や
`<home-manager/…>` のような import は flake の input に置き換えます。

生成された flake は、Rikka のビルドに使用された nixpkgs（NixOS unstable）に追従します。
安定版を使い続けたい場合は [nixpkgs の選び方](#nixpkgs-の選び方) を参照します。

### 2. Rikka の input を追加

`flake.nix` に input を追加します。

```nix
inputs = {
  nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  rikka.url = "github:project-rikka/packages";
};
```

`rikka.inputs.nixpkgs.follows` は設定しません。理由は
[nixpkgs の選び方](#nixpkgs-の選び方) を参照します。

### 3. モジュールと Rikka のディレクトリを追加

Rikka のテンプレートモジュールを入れた `rikka/` ディレクトリを作成します。

```sh
sudo mkdir /etc/nixos/rikka
cd /etc/nixos/rikka
sudo nix flake init -t github:project-rikka/packages#modules
```

次に、モジュールとディレクトリをシステムの `modules` に追加します。

```nix
outputs = { nixpkgs, rikka, ... }: {
  nixosConfigurations.laptop = nixpkgs.lib.nixosSystem {
    modules = [
      ./configuration.nix
      rikka.nixosModules.default
      ./rikka
    ];
  };
};
```

`./rikka` は `configuration.nix` の `imports` に記述してもかまいません。

### 4. Rikka の有効化とログイン方法の選択

`configuration.nix` に以下を追記します。

```nix
rikka.enable = true;
```

続いて Rikka の起動方法を選択します。

- **ディスプレイマネージャーを使う場合**
  GDM、SDDM、greetd などを使っている場合はそのまま残し、ログイン画面で **Rikka**
  セッションを選択します。
- **ディスプレイマネージャーを使わない場合**
  `rikka.autologin.user = "alice";` を設定すると、起動時にログイン画面を出さず、指定した
  ユーザーで tty1 に直接 Rikka を立ち上げます（Rikka には独自のロック画面があります）。
  競合を防ぐため、ほかのディスプレイマネージャーはあらかじめ無効にします。NixOS では
  `services.xserver.enable` が有効で、かつほかのディスプレイマネージャーが指定されていない
  場合に既定で LightDM が有効になりますが、`rikka.autologin.user` を設定するとこの既定動作は
  自動的に無効化されます。

### 5. リビルド

`/etc/nixos` が Git リポジトリの場合は、先に新しいファイルをステージングします。
flake は Git の追跡対象（tracked）になっているファイルしか認識しません。

```sh
cd /etc/nixos
sudo git add flake.nix flake.lock rikka
```

リビルドして再起動します。

```sh
sudo nixos-rebuild switch --flake /etc/nixos
```

初回のビルドでは Rikka のパッケージと、そのビルドに使われた nixpkgs をダウンロード
するため、完了まで時間がかかります。

## /etc/nixos 以外にある設定

Rikka の設定アプリと Packages アプリは、`rikka/` ディレクトリ内のモジュールを編集し、
root 権限で flake をリビルドします。それぞれのパスを Rikka に指定します。

```nix
rikka.configuration = {
  flake = "/home/alice/nixos";
  modules = "/home/alice/nixos/hosts/laptop/rikka";
};
```

`modules` の既定値は `<flake>/rikka` で、flake ディレクトリ内にある必要があります。
複数のマシンを管理する flake では、マシンごとにディレクトリを用意し、その中で
`nix flake init -t github:project-rikka/packages#modules` を実行して各マシンの設定から
import します。Rikka が書き込むファイルの所有者はディレクトリの所有者と一致するため、
ホームディレクトリ配下の設定ファイルの所有権はそのまま維持されます。

Rikka は `nixos-rebuild` と同様、マシンのホスト名と一致する名前の構成をリビルドします。
flake 内の構成が 1 つだけの場合は、名前に関係なくその構成が使用されます。

Rikka による NixOS 設定の自動編集を使わない場合は `rikka.configuration.flake = null;` を
設定します。設定アプリのシステム関連ページや Packages アプリには、Rikka が設定を管理して
いない旨が表示され、ファイルは一切変更されません。この場合 `rikka/` ディレクトリは不要です。

## nixpkgs の選び方

Rikka のパッケージはビルド済みであり、`github:project-rikka/packages` が固定している
nixpkgs のリビジョンにリンクされています。システム側でどの nixpkgs を使っていても、
Rikka 自体は常にそのライブラリを使用します。Mesa も同様で、AMD や Intel など Mesa で動く
GPU では Rikka は自前の Mesa で描画し、システム側のものを使うのは NVIDIA の
プロプライエタリドライバだけです。

- **NixOS unstable：** `nixpkgs.follows = "rikka/nixpkgs";` を指定します。システム側も
  Rikka のビルドに使われたものとまったく同じライブラリを使用するようになり、Mesa も
  1 つで共有されます。両方が同時に更新されるため、パッケージの再ビルドは不要です。
- **安定版：** 独自の `nixpkgs.url`（例：`github:NixOS/nixpkgs/nixos-26.05`）をそのまま
  使います。Rikka は独自のライブラリと Mesa を使い続け、システムのほかの部分には
  指定したリリースのグラフィックドライバが使われます。
- `rikka.inputs.nixpkgs.follows = "nixpkgs";` は**絶対に**設定しません。
  Rikka のビルド済みバイナリが別のライブラリにリンクし直され、動作しなくなる恐れがあります。

## アプリの選び方

モジュールは Rikka デスクトップとコアアプリを常にインストールします。それ以外のアプリは
オプション（任意）です。一覧は次のコマンドで確認できます。

```sh
nix eval --json github:project-rikka/packages#catalog
```

`offer` が `required` 以外の項目がオプションアプリです。

アプリは次のいずれかの方法で有効化します（同じアプリを両方で指定しません）。

- **Packages** アプリの **Rikka apps**（Rikka のアプリ）から選択します。選択内容が
  `rikka/apps.nix` に書き込まれ、自動的にリビルドされます。
- 自分の設定ファイルに直接記述します（例：`rikka.apps.music.enable = true;`）。

## Rikka が書き込む設定

設定アプリと Packages アプリは、次のオプションを `rikka/` 内のモジュールに書き込みます。
Rikka 側で設定を変更する前に、自分の設定ファイルにある同じオプションの定義を削除します。
同じ優先度で定義が重複すると、NixOS のビルド時に競合エラーが発生します。

| Rikka での場所 | オプション |
|---|---|
| ユーザー | `users.users.<you>.description` |
| システム | `networking.hostName`、`boot.loader.systemd-boot.configurationLimit`、`nix.gc.*`、`nix.optimise.automatic`、`nix.settings.experimental-features`、`nixpkgs.config.allowUnfree` |
| 日付と時刻 | `time.timeZone`、`time.hardwareClockInLocalTime`、`services.automatic-timezoned.enable`、`services.timesyncd.enable`、`networking.timeServers` |
| 言語と地域、キーボード | `i18n.defaultLocale`、`i18n.extraLocaleSettings.LC_MESSAGES`、`console.keyMap`、`services.xserver.xkb.layout`、`services.xserver.xkb.variant`、`i18n.inputMethod.fcitx5.addons` |
| 電源とバッテリー | `services.logind.settings.Login.*`、`services.upower.*` |
| リモートデスクトップ | `security.pam.services.rikka-remote`、ファイアウォールの UDP ポート |
| Packages | `environment.systemPackages`、インストールした `programs.*` と `services.*`、`rikka.apps.*` |

`environment.systemPackages` のようなリスト型のオプションは自分の定義とマージされるため、
競合しません。

## 入力メソッド

Rikka は fcitx5 を独自に起動し、変換候補を表示します。日本語、中国語、韓国語、ベトナム語を
入力するには、fcitx5 を有効化したうえで二重起動を防止します。

```nix
i18n.inputMethod = {
  enable = true;
  type = "fcitx5";
  fcitx5.waylandFrontend = true;
  fcitx5.addons = [ pkgs.fcitx5-mozc ];
};
systemd.user.units."app-org.fcitx.Fcitx5@autostart.service".enable = false;
```

ほかの fcitx5 アドオンは、設定アプリの **キーボード** から追加できます。

## グラフィックドライバ

Intel と AMD の GPU は NixOS の既定設定のまま動作します（Rikka が
`hardware.graphics.enable` を自動で有効にします）。

NVIDIA の場合は、オープンカーネルモジュールとモードセッティングを使用します。

```nix
services.xserver.videoDrivers = [ "nvidia" ];
hardware.nvidia = {
  open = true;
  modesetting.enable = true;
  package = config.boot.kernelPackages.nvidiaPackages.latest;
};
nixpkgs.config.allowUnfree = true;
```

## Rikka と競合するもの

- **PulseAudio**
  Rikka は PipeWire を使用します。`services.pulseaudio.enable = true;` は削除します。
  両方が有効だと NixOS のビルドがエラーになります。
- **TLP と auto-cpufreq**
  Rikka は、どちらも有効でない場合にのみ `power-profiles-daemon` を有効化します。
  いずれかが動作している間は、Rikka の電源プロファイル操作は利用できません。
- **`rikka.autologin.user` とほかのディスプレイマネージャーの併用**
  リビルドが停止し、競合の原因が表示されます。どちらか一方のみを使用します。

## 更新

Packages アプリは、`nix flake update` と同様に flake 全体を更新してリビルドします。
進捗とログは通知および Updates アプリに表示されます。

ターミナルから更新する場合は次のコマンドを実行します。

```sh
cd /etc/nixos
sudo nix flake update
sudo nixos-rebuild switch --flake /etc/nixos
```

Rikka のみを更新したい場合は `sudo nix flake update rikka` を実行します。

## Rikka の削除

1. `rikka.enable`、`rikka.autologin.user` など、`rikka.*` のオプションをすべて削除します。
2. `rikka.nixosModules.default` と `./rikka` を `modules` から除外します。
3. input `rikka` を削除します。`nixpkgs` が `rikka/nixpkgs` に追従している場合は、
   代わりに独自の `url` を指定します。
4. `rikka/` ディレクトリを削除してリビルドします。

## トラブルシューティング

**設定アプリに「Rikka does not manage this system's configuration」と表示される**
`rikka.configuration.flake` が `null` になっているか、Rikka のモジュールを含めずに
システムがビルドされています。オプションを設定してリビルドします。

**設定アプリに、ディレクトリが「does not hold Rikka's modules」と表示される**
`rikka.configuration.modules` で指定したディレクトリに `default.nix` がありません。
そのディレクトリ内で `nix flake init -t github:project-rikka/packages#modules` を実行し、
モジュールを作成します。

**リビルドが「path '…/rikka/default.nix' does not exist」で失敗する**
ファイルは存在するものの Git の追跡対象になっていません。flake 内で `git add rikka` を
実行します。

**セッションが起動してすぐコンソールに戻る**
`journalctl -b -u rikka-autologin`（ディスプレイマネージャーを使用している場合は
`journalctl -b --user`）でログを確認します。多くの場合、Vulkan 1.3 ドライバの
不足や、GPU が KMS に対応していないことが原因です。

## オプション一覧

| オプション | 既定値 | 説明 |
|---|---|---|
| `rikka.enable` | `false` | Rikka デスクトップをインストールして有効にします。 |
| `rikka.apps.<id>.enable` | `false` | オプションアプリ `<id>` をインストールします。 |
| `rikka.autologin.user` | `null` | 起動時にログイン画面を出さず、指定したユーザーで tty1 に Rikka を立ち上げます。 |
| `rikka.configuration.flake` | `"/etc/nixos"` | Rikka がリビルドする flake のパス。`null` で NixOS 設定の自動編集を無効にします。 |
| `rikka.configuration.modules` | `"<flake>/rikka"` | Rikka が書き込むモジュールを配置する、flake 内のディレクトリ。 |