package sh.rikka.client.hud;

import com.mojang.blaze3d.platform.InputConstants;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Optional;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;
import os.rikka.Align;
import os.rikka.ButtonVariant;
import os.rikka.Color;
import os.rikka.ColorRole;
import os.rikka.FontWeight;
import os.rikka.Justification;
import os.rikka.Rikka;
import os.rikka.TextAlignment;
import os.rikka.TextRole;
import os.rikka.TextTone;
import os.rikka.TransitionStyle;
import os.rikka.View;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.menu.Labels;
import sh.rikka.client.menu.MenuState;
import sh.rikka.client.menu.ModMenuScreen;
import sh.rikka.client.menu.Page;
import sh.rikka.client.mod.HudMod;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.DoubleSetting;
import sh.rikka.client.ui.RikkaScreen;
import sh.rikka.client.ui.SurfaceScale;
import sh.rikka.client.ui.style.Colors;

public final class HudEditorScreen extends RikkaScreen {
	private static final float FRAME_BORDER = 1.0F;
	private static final float ACTIVE_BORDER = 1.5F;
	private static final float RESTING_FILL = 0.06F;
	private static final float HOVERED_FILL = 0.14F;
	private static final float SELECTED_FILL = 0.22F;
	private static final float RESTING_OUTLINE = 0.55F;
	private static final float GUIDE_WIDTH = 1.0F;
	private static final float TOOLBAR_MARGIN = Rikka.SPACE_LOOSE;
	private static final float SCALE_SLIDER_WIDTH = 140.0F;
	private static final float SCALE_VALUE_WIDTH = 44.0F;
	private static final float NAME_WIDTH = 140.0F;
	private static final float NUDGE = 1.0F;
	private static final float SHIFTED_NUDGE = 10.0F;

	private final MenuState menu;
	private final boolean returnsToMenu;
	private Optional<HudMod> selected;
	private Optional<HudMod> hovered = Optional.empty();
	private @Nullable Grab grab;
	private Snapping.Snap guides = Snapping.Snap.free(0.0F, 0.0F);

	private HudEditorScreen(MenuState menu, Optional<HudMod> focus, boolean returnsToMenu) {
		super(Component.translatable(RikkaClient.MOD_ID + ".menu.hud.title"), SurfaceScale.HUD);
		this.menu = menu;
		this.selected = focus;
		this.returnsToMenu = returnsToMenu;
	}

	public static HudEditorScreen standalone(MenuState menu) {
		return new HudEditorScreen(menu, Optional.empty(), false);
	}

	public static HudEditorScreen fromMenu(MenuState menu, Optional<HudMod> focus) {
		return new HudEditorScreen(menu, focus, true);
	}

	private record Grab(HudMod mod, float offsetX, float offsetY) {
	}

	@Override
	protected void view(View view) {
		if (view.isLeaving()) {
			return;
		}
		float width = surfaceWidth();
		float height = surfaceHeight();
		Color accent = view.paletteColor(ColorRole.ACCENT);
		Color text = view.paletteColor(ColorRole.TEXT);
		for (HudBox box : boxes(width, height)) {
			frame(view, box, accent, text);
		}
		if (this.grab != null) {
			this.guides.guideX().ifPresent(x -> view.box(() -> {
				})
				.position(x - GUIDE_WIDTH / 2.0F, 0.0F)
				.width(GUIDE_WIDTH)
				.height(height)
				.background(accent)
				.key("guide:x")
				.transition(TransitionStyle.FADE));
			this.guides.guideY().ifPresent(y -> view.box(() -> {
				})
				.position(0.0F, y - GUIDE_WIDTH / 2.0F)
				.width(width)
				.height(GUIDE_WIDTH)
				.background(accent)
				.key("guide:y")
				.transition(TransitionStyle.FADE));
		}
		view.column(() -> toolbar(view))
			.position(0.0F, 0.0F)
			.width(width)
			.height(height)
			.justify(Justification.END)
			.alignItems(Align.CENTER)
			.paddingEdges(0.0F, 0.0F, TOOLBAR_MARGIN, 0.0F)
			.key("toolbar")
			.transition(TransitionStyle.SLIDE_FROM_BOTTOM);
	}

	private void frame(View view, HudBox box, Color accent, Color text) {
		boolean isSelected = this.selected.equals(Optional.of(box.mod()));
		boolean isHovered = this.hovered.equals(Optional.of(box.mod()));
		float fill = isSelected ? SELECTED_FILL : isHovered ? HOVERED_FILL : RESTING_FILL;
		boolean active = isSelected || isHovered;
		view.box(() -> {
			})
			.position(box.left(), box.top())
			.width(box.width())
			.height(box.height())
			.background(Colors.withAlpha(accent, fill))
			.border(active ? ACTIVE_BORDER : FRAME_BORDER, active ? accent : Colors.withAlpha(text, RESTING_OUTLINE))
			.cornerRadius(Rikka.CORNER_DETAIL)
			.key("frame:" + box.mod().id())
			.transition(TransitionStyle.FADE);
	}

	private void toolbar(View view) {
		view.box(() -> view.row(() -> {
					Optional<HudMod> chosen = this.selected.filter(Mod::isEnabled);
					if (chosen.isPresent()) {
						inspector(view, chosen.get());
					} else {
						view.icon("dashboard_customize").tint(view.paletteColor(ColorRole.ACCENT));
						view.text(Labels.of("hud.title")).textWeight(FontWeight.SEMIBOLD).textNoWrap();
					}
					view.verticalDivider();
					if (this.returnsToMenu) {
						view.button(Labels.of("hud.mods"))
							.buttonStyle(ButtonVariant.SECONDARY)
							.onPress(context -> leaveTo(new ModMenuScreen(this.menu)));
					}
					view.button(Labels.of("hud.done"))
						.onPress(context -> onClose());
				})
				.gap(Rikka.SPACE_NORMAL)
				.alignItems(Align.CENTER))
			.surface()
			.border(1.0F, Colors.withAlpha(view.paletteColor(ColorRole.OUTLINE), RESTING_OUTLINE))
			.cornerRadius(Rikka.CORNER_PANEL)
			.paddingEdges(Rikka.SPACE_SNUG, Rikka.SPACE_SNUG, Rikka.SPACE_SNUG, Rikka.SPACE_ROOMY);
	}

	private void inspector(View view, HudMod mod) {
		DoubleSetting scale = mod.scale();
		view.icon(mod.icon()).tint(view.paletteColor(ColorRole.ACCENT));
		view.text(mod.name().getString()).textWeight(FontWeight.SEMIBOLD).textNoWrap().width(NAME_WIDTH);
		view.text(Labels.of("hud.scale")).textStyle(TextRole.CAPTION).tone(TextTone.MUTED);
		view.slider(scale.get())
			.range(scale.min(), scale.max())
			.step(scale.step())
			.width(SCALE_SLIDER_WIDTH)
			.onValue((context, value) -> scale.set(value));
		view.text(String.format(Locale.ROOT, "%.2f×", scale.get()))
			.textStyle(TextRole.LABEL)
			.tone(TextTone.MUTED)
			.textAlign(TextAlignment.RIGHT)
			.width(SCALE_VALUE_WIDTH);
		view.iconButton("restart_alt")
			.tooltip(Labels.of("hud.reset"))
			.onPress(context -> {
				mod.position().reset();
				scale.reset();
			});
		view.iconButton("tune")
			.tooltip(Labels.of("hud.settings"))
			.onPress(context -> openSettings(mod));
	}

	private List<HudBox> boxes(float width, float height) {
		List<HudBox> boxes = new ArrayList<>();
		for (Mod mod : RikkaClient.mods().all()) {
			if (mod instanceof HudMod hud && hud.isEnabled()) {
				boxes.add(HudBox.of(hud, width, height));
			}
		}
		return boxes;
	}

	private Optional<HudBox> hit(float x, float y) {
		List<HudBox> boxes = boxes(surfaceWidth(), surfaceHeight());
		for (int index = boxes.size() - 1; index >= 0; index--) {
			if (boxes.get(index).contains(x, y)) {
				return Optional.of(boxes.get(index));
			}
		}
		return Optional.empty();
	}

	private void openSettings(HudMod mod) {
		this.menu.open(new Page.ModSettings(mod));
		leaveTo(new ModMenuScreen(this.menu));
	}

	@Override
	protected boolean pressed(float x, float y, MouseButtonEvent event) {
		Optional<HudBox> target = hit(x, y);
		switch (event.button()) {
			case InputConstants.MOUSE_BUTTON_LEFT -> {
				this.selected = target.map(HudBox::mod);
				this.grab = target.map(box -> new Grab(box.mod(), x - box.left(), y - box.top())).orElse(null);
				this.guides = Snapping.Snap.free(0.0F, 0.0F);
			}
			case InputConstants.MOUSE_BUTTON_RIGHT -> target.ifPresent(box -> openSettings(box.mod()));
			default -> {
				return false;
			}
		}
		refresh();
		return true;
	}

	@Override
	protected void moved(float x, float y, boolean used) {
		Grab held = this.grab;
		if (held == null) {
			Optional<HudMod> under = used ? Optional.empty() : hit(x, y).map(HudBox::mod);
			if (!under.equals(this.hovered)) {
				this.hovered = under;
				refresh();
			}
			return;
		}
		float width = surfaceWidth();
		float height = surfaceHeight();
		HudBox box = HudBox.of(held.mod(), width, height);
		float left = x - held.offsetX();
		float top = y - held.offsetY();
		this.guides = shiftHeld()
			? Snapping.clamp(box, left, top, width, height)
			: Snapping.snap(box, left, top, others(box.mod(), width, height), width, height);
		box.mod().position().set(Snapping.position(box, this.guides.left(), this.guides.top(), width, height));
		refresh();
	}

	@Override
	protected boolean released(float x, float y, MouseButtonEvent event) {
		if (this.grab == null || event.button() != InputConstants.MOUSE_BUTTON_LEFT) {
			return false;
		}
		this.grab = null;
		refresh();
		return true;
	}

	@Override
	protected boolean scrolled(float x, float y, double amount) {
		Optional<HudMod> target = hit(x, y).map(HudBox::mod).or(() -> this.selected);
		if (target.isEmpty()) {
			return false;
		}
		DoubleSetting scale = target.get().scale();
		scale.set(scale.get() + Math.signum(amount) * scale.step());
		refresh();
		return true;
	}

	@Override
	public boolean keyPressed(KeyEvent event) {
		Optional<HudMod> chosen = this.selected.filter(Mod::isEnabled);
		if (chosen.isPresent() && !isLeaving()) {
			float step = event.hasShiftDown() ? SHIFTED_NUDGE : NUDGE;
			float dx = switch (event.key()) {
				case InputConstants.KEY_LEFT -> -step;
				case InputConstants.KEY_RIGHT -> step;
				default -> 0.0F;
			};
			float dy = switch (event.key()) {
				case InputConstants.KEY_UP -> -step;
				case InputConstants.KEY_DOWN -> step;
				default -> 0.0F;
			};
			if (dx != 0.0F || dy != 0.0F) {
				nudge(chosen.get(), dx, dy);
				return true;
			}
		}
		return super.keyPressed(event);
	}

	private void nudge(HudMod mod, float dx, float dy) {
		float width = surfaceWidth();
		float height = surfaceHeight();
		HudBox box = HudBox.of(mod, width, height);
		Snapping.Snap moved = Snapping.clamp(box, box.left() + dx, box.top() + dy, width, height);
		mod.position().set(Snapping.position(box, moved.left(), moved.top(), width, height));
		refresh();
	}

	private List<HudBox> others(HudMod moving, float width, float height) {
		return boxes(width, height).stream().filter(box -> box.mod() != moving).toList();
	}

	private static boolean shiftHeld() {
		return InputConstants.isKeyDown(InputConstants.KEY_LSHIFT) || InputConstants.isKeyDown(InputConstants.KEY_RSHIFT);
	}

	@Override
	public void onClose() {
		leaveTo(this.returnsToMenu ? new ModMenuScreen(this.menu) : null);
	}
}
