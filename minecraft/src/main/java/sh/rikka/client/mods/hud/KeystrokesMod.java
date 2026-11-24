package sh.rikka.client.mods.hud;

import com.mojang.blaze3d.platform.InputConstants;
import java.util.List;
import java.util.function.BooleanSupplier;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.client.resources.language.I18n;
import org.jspecify.annotations.Nullable;
import os.rikka.Align;
import os.rikka.Color;
import os.rikka.Element;
import os.rikka.FontWeight;
import os.rikka.Justification;
import os.rikka.View;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.InputAction;
import sh.rikka.client.events.MouseInputEvent;
import sh.rikka.client.mod.HudMod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.ColorSetting;
import sh.rikka.client.setting.IntSetting;
import sh.rikka.client.setting.ScreenPosition;
import sh.rikka.client.ui.Smoothed;
import sh.rikka.client.ui.style.Colors;

public final class KeystrokesMod extends HudMod {
	private static final float KEY = 34.0F;
	private static final float GAP = 3.0F;
	private static final float WIDTH = KEY * 3.0F + GAP * 2.0F;
	private static final float MOUSE_HEIGHT = 30.0F;
	private static final float MOUSE_HEIGHT_WITH_RATE = 38.0F;
	private static final float SPACE_HEIGHT = 20.0F;
	private static final float SPACE_BAR_WIDTH = 36.0F;
	private static final float SPACE_BAR_HEIGHT = 2.0F;
	private static final float CORNER_RADIUS = 7.0F;
	private static final float KEY_TEXT_SIZE = 14.0F;
	private static final float MOUSE_TEXT_SIZE = 12.0F;
	private static final float RATE_TEXT_SIZE = 10.0F;
	private static final float SETTLE_EPSILON = 0.002F;

	private final BoolSetting labels = bool("labels", true);
	private final BoolSetting space = bool("space", true);
	private final BoolSetting mouse = bool("mouse", true);
	private final BoolSetting clickRate = bool("cps", true).visibleWhen(this.mouse::get);
	private final BoolSetting background = bool("background", true);
	private final ColorSetting pressedColor = color("pressed_color", 0xD9FFFFFF);
	private final ColorSetting textColor = color("text_color", 0xFFFFFFFF);
	private final ColorSetting pressedTextColor = color("pressed_text_color", 0xFF101010);
	private final IntSetting fadeTime = integer("fade_time", 60, 10, 400);
	private final ClickRates clicks = new ClickRates();
	private final Key upKey = new Key(() -> options().keyUp.isDown());
	private final Key leftKey = new Key(() -> options().keyLeft.isDown());
	private final Key downKey = new Key(() -> options().keyDown.isDown());
	private final Key rightKey = new Key(() -> options().keyRight.isDown());
	private final Key spaceBar = new Key(() -> options().keyJump.isDown());
	private final Key leftButton = new Key(() -> false);
	private final Key rightButton = new Key(() -> false);
	private final List<Key> keys = List.of(
		this.upKey,
		this.leftKey,
		this.downKey,
		this.rightKey,
		this.spaceBar,
		this.leftButton,
		this.rightButton
	);

	public KeystrokesMod(ScreenPosition defaultPosition) {
		super("keystrokes", defaultPosition, "keyboard");
	}

	@Override
	protected void onDisable() {
		this.clicks.clear();
		for (Key key : this.keys) {
			key.reset();
		}
	}

	@EventHandler(receiveCancelled = true)
	private void onMouseInput(MouseInputEvent event) {
		this.clicks.record(event);
		if (Minecraft.getInstance().gui.screen() != null && event.action() == InputAction.PRESS) {
			return;
		}
		@Nullable Key key = switch (event.input().button()) {
			case InputConstants.MOUSE_BUTTON_LEFT -> this.leftButton;
			case InputConstants.MOUSE_BUTTON_RIGHT -> this.rightButton;
			default -> null;
		};
		if (key == null) {
			return;
		}
		switch (event.action()) {
			case PRESS -> key.press();
			case RELEASE -> key.release();
			case REPEAT -> {
			}
		}
	}

	@Override
	public float width() {
		return WIDTH;
	}

	@Override
	public float height() {
		float height = KEY * 2.0F + GAP;
		if (this.mouse.get()) {
			height += GAP + mouseHeight();
		}
		if (this.space.get()) {
			height += GAP + SPACE_HEIGHT;
		}
		return height;
	}

	@Override
	public boolean animating() {
		return this.upKey.isChanging()
			|| this.leftKey.isChanging()
			|| this.downKey.isChanging()
			|| this.rightKey.isChanging()
			|| this.mouse.get() && (this.leftButton.isChanging() || this.rightButton.isChanging())
			|| this.space.get() && this.spaceBar.isChanging();
	}

	@Override
	protected void view(View view) {
		float fadeMillis = this.fadeTime.get();
		Options options = options();
		keyboardKey(view, this.upKey, options.keyUp, KEY + GAP, 0.0F, fadeMillis);
		float row = KEY + GAP;
		keyboardKey(view, this.leftKey, options.keyLeft, 0.0F, row, fadeMillis);
		keyboardKey(view, this.downKey, options.keyDown, KEY + GAP, row, fadeMillis);
		keyboardKey(view, this.rightKey, options.keyRight, (KEY + GAP) * 2.0F, row, fadeMillis);
		float top = row + KEY + GAP;
		if (this.mouse.get()) {
			float half = (WIDTH - GAP) / 2.0F;
			mouseKey(view, this.leftButton, "left", this.clicks.left(), 0.0F, top, half, fadeMillis);
			mouseKey(view, this.rightButton, "right", this.clicks.right(), half + GAP, top, half, fadeMillis);
			top += mouseHeight() + GAP;
		}
		if (this.space.get()) {
			spaceKey(view, top, fadeMillis);
		}
	}

	private void keyboardKey(View view, Key key, KeyMapping mapping, float x, float y, float fadeMillis) {
		float fade = key.fade(fadeMillis);
		Color tint = tint(fade);
		Element element = keyBox(view, () -> {
			if (this.labels.get()) {
				view.text(mapping.getTranslatedKeyMessage().getString()).textSize(KEY_TEXT_SIZE).textWeight(FontWeight.MEDIUM).textNoWrap().tint(tint);
			}
		}, fade);
		element.position(x, y).width(KEY).height(KEY);
	}

	private void mouseKey(View view, Key key, String side, int clicksPerSecond, float x, float y, float width, float fadeMillis) {
		float fade = key.fade(fadeMillis);
		Color tint = tint(fade);
		Element element = keyBox(view, () -> {
			if (this.labels.get()) {
				view.text(I18n.get(translationKey() + "." + side)).textSize(MOUSE_TEXT_SIZE).textWeight(FontWeight.MEDIUM).textNoWrap().tint(tint);
			}
			if (this.clickRate.get()) {
				view.text(I18n.get(translationKey() + ".cps", clicksPerSecond)).textSize(RATE_TEXT_SIZE).textNoWrap().tint(tint);
			}
		}, fade);
		element.position(x, y).width(width).height(mouseHeight());
	}

	private void spaceKey(View view, float y, float fadeMillis) {
		float fade = this.spaceBar.fade(fadeMillis);
		Color tint = tint(fade);
		Element element = keyBox(view, () -> {
			if (this.labels.get()) {
				view.box(() -> {
				}).width(SPACE_BAR_WIDTH).height(SPACE_BAR_HEIGHT).cornerRadius(SPACE_BAR_HEIGHT / 2.0F).background(tint);
			}
		}, fade);
		element.position(0.0F, y).width(WIDTH).height(SPACE_HEIGHT);
	}

	private Element keyBox(View view, Runnable content, float fade) {
		Color pressed = Colors.of(this.pressedColor.argb());
		Element element = view.box(() -> view.column(content).fillWidth().fillHeight().alignItems(Align.CENTER).justify(Justification.CENTER))
			.cornerRadius(CORNER_RADIUS);
		if (this.background.get()) {
			element.glass().background(Colors.mix(BACKDROP, pressed, fade));
		} else {
			element.background(Colors.withAlpha(pressed, pressed.a() * fade));
		}
		return element;
	}

	private Color tint(float fade) {
		return Colors.mix(Colors.of(this.textColor.argb()), Colors.of(this.pressedTextColor.argb()), fade);
	}

	private float mouseHeight() {
		return this.clickRate.get() ? MOUSE_HEIGHT_WITH_RATE : MOUSE_HEIGHT;
	}

	private static Options options() {
		return Minecraft.getInstance().options;
	}

	private static final class Key {
		private final BooleanSupplier held;
		private final Smoothed fade = new Smoothed(1.0F, SETTLE_EPSILON, 0.0F);
		private boolean pressed;
		private boolean pressedSinceView;
		private boolean shown;

		Key(BooleanSupplier held) {
			this.held = held;
		}

		void press() {
			this.pressed = true;
			this.pressedSinceView = true;
		}

		void release() {
			this.pressed = false;
		}

		void reset() {
			this.pressed = false;
			this.pressedSinceView = false;
			this.shown = false;
			this.fade.snap(0.0F);
		}

		boolean isDown() {
			return this.pressed || this.pressedSinceView || this.held.getAsBoolean();
		}

		boolean isChanging() {
			return isDown() != this.shown || !this.fade.isSettled();
		}

		float fade(float timeConstantMillis) {
			this.shown = isDown();
			this.pressedSinceView = false;
			return this.fade.update(this.shown ? 1.0F : 0.0F, timeConstantMillis);
		}
	}
}
