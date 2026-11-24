package sh.rikka.client.menu;

import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;
import os.rikka.Align;
import os.rikka.Color;
import os.rikka.ColorRole;
import os.rikka.Justification;
import os.rikka.Rikka;
import os.rikka.TextRole;
import os.rikka.TransitionStyle;
import os.rikka.View;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.hud.HudEditorScreen;
import sh.rikka.client.menu.settings.KeyCapture;
import sh.rikka.client.menu.settings.SettingControls;
import sh.rikka.client.menu.settings.SettingRows;
import sh.rikka.client.ui.RikkaScreen;
import sh.rikka.client.ui.style.Colors;

public final class ModMenuScreen extends RikkaScreen {
	private static final float PANEL_MAX_WIDTH = 1120.0F;
	private static final float PANEL_MAX_HEIGHT = 720.0F;
	private static final float MARGIN = Rikka.SPACE_WIDE;
	private static final float TOP_BAR_HEIGHT = 60.0F;
	private static final float SEARCH_WIDTH = 240.0F;
	private static final float SIDEBAR_TINT = 0.5F;
	private static final float BORDER_ALPHA = 0.4F;
	private static final int FIRST_MOD_PAGE = 4;

	private final MenuState state;
	private final SettingControls controls = new SettingControls();
	private final Sidebar sidebar = new Sidebar();
	private final ModPage modPage = new ModPage(this.controls);
	private final ProfilesPage profiles = new ProfilesPage(this.controls::forgetAll);
	private final ThemesPage themes = new ThemesPage();
	private final PreferencesPage preferences = new PreferencesPage(new SettingRows(this.controls));
	private Page shown;

	public ModMenuScreen(MenuState state) {
		super(Component.translatable(RikkaClient.MOD_ID + ".menu.title"));
		this.state = state;
		this.shown = state.page();
	}

	@Override
	protected void view(View view) {
		if (view.isLeaving()) {
			return;
		}
		Page page = this.state.page();
		if (!page.equals(this.shown)) {
			this.shown = page;
			this.controls.forgetAll();
		}
		float width = Math.clamp(surfaceWidth() - MARGIN * 2.0F, 0.0F, PANEL_MAX_WIDTH);
		float height = Math.clamp(surfaceHeight() - MARGIN * 2.0F, 0.0F, PANEL_MAX_HEIGHT);
		view.scrim();
		view.column(() -> panel(view, page, width, height))
			.fillWidth()
			.fillHeight()
			.alignItems(Align.CENTER)
			.justify(Justification.CENTER)
			.transition(TransitionStyle.FADE_SCALE);
	}

	private void panel(View view, Page page, float width, float height) {
		Color background = view.paletteColor(ColorRole.BACKGROUND);
		Color surface = view.paletteColor(ColorRole.SURFACE);
		view.box(() -> view.row(() -> {
					view.box(() -> this.sidebar.view(view, this.state, height, canEditHud(), this::editHud))
						.background(Colors.mix(background, surface, SIDEBAR_TINT))
						.height(height);
					view.verticalDivider();
					view.column(() -> content(view, page))
						.grow(1.0F)
						.height(height);
				})
				.width(width)
				.height(height))
			.background(background)
			.border(1.0F, Colors.withAlpha(view.paletteColor(ColorRole.OUTLINE), BORDER_ALPHA))
			.cornerRadius(Rikka.CORNER_WINDOW)
			.clipContent()
			.width(width)
			.height(height);
	}

	private void content(View view, Page page) {
		topBar(view, page);
		view.divider();
		view.pageStack(pageNumber(page), () -> view.scroll(() -> view.column(() -> page(view, page))
						.gap(Rikka.SPACE_ROOMY)
						.padding(Rikka.SPACE_LOOSE)
						.fillWidth())
					.fillWidth()
					.fillHeight())
			.grow(1.0F)
			.fillWidth();
	}

	private void topBar(View view, Page page) {
		view.row(() -> {
				view.iconButton("arrow_back")
					.tooltip(Labels.of("back"))
					.enabled(this.state.canGoBack())
					.onPress(context -> this.state.back());
				view.iconButton("arrow_forward")
					.tooltip(Labels.of("forward"))
					.enabled(this.state.canGoForward())
					.onPress(context -> this.state.forward());
				view.text(title(page)).textStyle(TextRole.HEADING).textNoWrap().grow(1.0F).shrink(1.0F);
				if (page instanceof Page.Mods) {
					view.textField(this.state.search())
						.placeholder(Labels.of("search"))
						.width(SEARCH_WIDTH)
						.key("search")
						.transition(TransitionStyle.FADE)
						.onChange(context -> {
						});
				}
				view.iconButton("close")
					.tooltip(Labels.of("close"))
					.onPress(context -> onClose());
			})
			.gap(Rikka.SPACE_SNUG)
			.alignItems(Align.CENTER)
			.paddingEdges(0.0F, Rikka.SPACE_ROOMY, 0.0F, Rikka.SPACE_ROOMY)
			.height(TOP_BAR_HEIGHT)
			.fillWidth();
	}

	private void page(View view, Page page) {
		switch (page) {
			case Page.Mods _ -> ModsPage.view(view, this.state);
			case Page.ModSettings settings -> this.modPage.view(view, settings.mod());
			case Page.Profiles _ -> this.profiles.view(view);
			case Page.Themes _ -> this.themes.view(view);
			case Page.Preferences _ -> this.preferences.view(view);
		}
	}

	private static String title(Page page) {
		return switch (page) {
			case Page.Mods _ -> Labels.of("nav.mods");
			case Page.ModSettings settings -> settings.mod().name().getString();
			case Page.Profiles _ -> Labels.of("nav.profiles");
			case Page.Themes _ -> Labels.of("nav.themes");
			case Page.Preferences _ -> Labels.of("nav.preferences");
		};
	}

	private static int pageNumber(Page page) {
		return switch (page) {
			case Page.Mods _ -> 0;
			case Page.Profiles _ -> 1;
			case Page.Themes _ -> 2;
			case Page.Preferences _ -> 3;
			case Page.ModSettings settings -> FIRST_MOD_PAGE + RikkaClient.mods().all().indexOf(settings.mod());
		};
	}

	private boolean canEditHud() {
		return this.minecraft.level != null;
	}

	private void editHud() {
		leaveTo(HudEditorScreen.fromMenu(this.state));
	}

	@Override
	public boolean keyPressed(KeyEvent event) {
		KeyCapture capture = this.controls.capture();
		if (!capture.isActive() || isLeaving()) {
			return super.keyPressed(event);
		}
		if (event.isEscape()) {
			capture.cancel();
		} else {
			capture.accept(InputConstants.getKey(event));
		}
		refresh();
		return true;
	}

	@Override
	public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
		KeyCapture capture = this.controls.capture();
		if (!capture.isActive() || isLeaving() || event.button() == InputConstants.MOUSE_BUTTON_LEFT) {
			return super.mouseClicked(event, doubleClick);
		}
		capture.accept(InputConstants.Type.MOUSE.getOrCreate(event.button()));
		refresh();
		return true;
	}

	@Override
	public void removed() {
		super.removed();
		this.controls.close();
		this.profiles.close();
		this.sidebar.close();
		this.themes.close();
	}
}
