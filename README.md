# RUST Simple HTTP Server
Rustの勉強のために作成しているシンプルなHTTPサーバー


### 利用方法(OpenWRT)
[ローカルフィードの作成手順](https://openwrt.org/docs/guide-developer/toolchain/use-buildsystem#creating_a_local_feed)に則り作業します。
1. [openwrt](https://github.com/openwrt/openwrt)をクローン
2. openwrt以下に適当なディレクトリ`my-packages`を作りその直下に`rust-simple-http-server/Makefile`を作成 
3. `openwrt/feeds.conf.default`に`src-link my-packages <ABSOLUTE-PATH/TO/my-packages>`を追加
4.  `./scripts/feeds update my_packages`
5. `./scripts/feeds install -a -p my_packages`
6. `make menuconfig`で`my-packages`からこのパッケージを選択
7. `make package/rust-simple-http-server/compile`
8. ipkファイルを転送して`opkg install xxx.ipk`

#### Makefile
```Makefile
include $(TOPDIR)/rules.mk

PKG_NAME:=rust-simple-http-server
PKG_VERSION:=0.1.1
PKG_RELEASE:=0

PKG_SOURCE:=$(PKG_NAME)-v$(PKG_VERSION).tar.gz
PKG_SOURCE_URL:=https://github.com/watosar/$(PKG_NAME)/archive/refs/tags/v$(PKG_VERSION).tar.gz?
PKG_HASH:=f2831da9654d59718b27c9ec44f08d82c695d6b6f37e32d4c3a2d54e4de3e440


PKG_BUILD_DEPENDS:=rust/host
PKG_BUILD_PARALLEL:=1

include $(INCLUDE_DIR)/package.mk
include $(TOPDIR)/feeds/packages/lang/rust/rust-package.mk

define Package/rust-simple-http-server
	SECTION:=my-package
	CATEGORY:=MyPackage
	TITLE:=Interactive processes viewer
	DEPENDS:=$(RUST_ARCH_DEPENDS)
	URL:=https://github.com/watosar/rust-simple-http-server
endef

define Package/rust-simple-http-server/install
	$(INSTALL_DIR) $(1)/usr/bin
	$(INSTALL_BIN) $(PKG_INSTALL_DIR)/bin/rust-simple-http-server $(1)/usr/bin
	$(INSTALL_DIR) $(1)/etc/init.d
	$(INSTALL_BIN) $(PKG_BUILD_DIR)/openwrt/etc/init.d/rust-simple-http-server $(1)/etc/init.d/rust-simple-http-server
	$(INSTALL_DIR) $(1)/etc/config
	$(CP) $(PKG_BUILD_DIR)/openwrt/etc/config/rust-simple-http-server $(1)/etc/config/rust-simple-http-server
	$(INSTALL_DIR) $(1)/tmp/www/html
	$(CP) -r $(PKG_BUILD_DIR)/openwrt/tmp/www/html $(1)/tmp/www/
endef

$(eval $(call RustBinPackage,rust-simple-http-server))
$(eval $(call BuildPackage,rust-simple-http-server))
```
