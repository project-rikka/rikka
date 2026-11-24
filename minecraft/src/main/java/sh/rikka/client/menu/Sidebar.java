package sh.rikka.client.menu;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.util.List;
import net.fabricmc.loader.api.FabricLoader;
import os.rikka.Align;
import os.rikka.ButtonVariant;
import os.rikka.ColorRole;
import os.rikka.Fit;
import os.rikka.FontWeight;
import os.rikka.Image;
import os.rikka.Rikka;
import os.rikka.TextRole;
import os.rikka.TextTone;
import os.rikka.View;
import sh.rikka.client.RikkaClient;

final class Sidebar implements AutoCloseable {
	private static final float WIDTH = 236.0F;
	private static final float LOGO_SIZE = 38.0F;
	private static final String LOGO = "/assets/" + RikkaClient.MOD_ID + "/textures/gui/project-rikka.jpg";
	private static final List<Entry> NAV = List.of(
		new Header("nav.client"),
		new Item("nav.mods", "extension", new Page.Mods()),
		new Item("nav.music", "library_music", new Page.Music()),
		new Item("nav.profiles", "switch_account", new Page.Profiles()),
		new Header("nav.personalization"),
		new Item("nav.themes", "palette", new Page.Themes()),
		new Item("nav.hud", "brush", new Page.Hud()),
		new Item("nav.preferences", "tune", new Page.Preferences())
	);

	private sealed interface Entry {
	}

	private record Header(String label) implements Entry {
	}

	private record Item(String label, String icon, Page page) implements Entry {
	}

	private final String version = FabricLoader.getInstance()
		.getModContainer(RikkaClient.MOD_ID)
		.map(container -> container.getMetadata().getVersion().getFriendlyString())
		.orElseThrow(() -> new IllegalStateException(RikkaClient.MOD_ID + " is not loaded"));
	private final Image logo = Image.fromEncoded(logoBytes());

	void view(View view, MenuState state, float height, boolean canEditHud, Runnable editHud) {
		view.column(() -> {
				view.row(() -> {
						view.imageView(this.logo)
							.width(LOGO_SIZE)
							.height(LOGO_SIZE)
							.imageFit(Fit.COVER)
							.cornerRadius(Rikka.CORNER_CONTROL);
						view.column(() -> {
								view.text(RikkaClient.NAME).textStyle(TextRole.SUBHEADING).textWeight(FontWeight.BOLD).textNoWrap();
								view.text(this.version).textStyle(TextRole.CAPTION).tone(TextTone.MUTED);
							})
							.gap(Rikka.SPACE_HAIR);
					})
					.gap(Rikka.SPACE_NORMAL)
					.alignItems(Align.CENTER)
					.paddingEdges(Rikka.SPACE_SNUG, Rikka.SPACE_SNUG, Rikka.SPACE_ROOMY, Rikka.SPACE_SNUG);
				view.navList(selected(state.page()), () -> {
						for (Entry entry : NAV) {
							switch (entry) {
								case Header header -> view.navHeader(Labels.of(header.label()));
								case Item item -> view.navItem(Labels.of(item.label())).leadingIcon(item.icon());
							}
						}
					})
					.fillWidth()
					.onSelect((context, index) -> state.open(page(index)));
				view.spacer();
				view.customButton(() -> {
						view.icon("dashboard_customize").tint(view.paletteColor(ColorRole.ON_ACCENT));
						view.text(Labels.of("nav.edit_hud")).tint(view.paletteColor(ColorRole.ON_ACCENT)).textNoWrap();
					})
					.buttonStyle(ButtonVariant.PRIMARY)
					.enabled(canEditHud)
					.fillWidth()
					.onPress(context -> editHud.run());
			})
			.gap(Rikka.SPACE_SNUG)
			.padding(Rikka.SPACE_ROOMY)
			.width(WIDTH)
			.height(height);
	}

	private static int selected(Page page) {
		Page section = page instanceof Page.ModSettings ? new Page.Mods() : page;
		for (int index = 0; index < NAV.size(); index++) {
			if (NAV.get(index) instanceof Item item && item.page().equals(section)) {
				return index;
			}
		}
		throw new IllegalStateException(page + " has no navigation entry");
	}

	private static Page page(int index) {
		return switch (NAV.get(index)) {
			case Item item -> item.page();
			case Header header -> throw new IllegalArgumentException(header.label() + " is a heading");
		};
	}

	private static byte[] logoBytes() {
		try (InputStream stream = Sidebar.class.getResourceAsStream(LOGO)) {
			if (stream == null) {
				throw new IllegalStateException("missing " + LOGO);
			}
			return stream.readAllBytes();
		} catch (IOException failure) {
			throw new UncheckedIOException("failed to read " + LOGO, failure);
		}
	}

	@Override
	public void close() {
		this.logo.close();
	}
}
