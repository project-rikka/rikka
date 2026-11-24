package sh.rikka.client.hud.style;

import java.util.List;
import java.util.Optional;
import java.util.stream.Stream;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.ColorSetting;
import sh.rikka.client.setting.ColorValue;
import sh.rikka.client.setting.DoubleSetting;
import sh.rikka.client.setting.EnumSetting;
import sh.rikka.client.setting.Setting;
import sh.rikka.client.setting.Settings;

public final class HudStyle {
	private static final double MAX_OUTLINE_WIDTH = 4.0;
	private static final double MAX_SHADOW_SIZE = 32.0;
	private static final double MAX_RADIUS = 14.0;
	private static final double MIN_SPEED = 0.1;
	private static final double MAX_SPEED = 4.0;
	private static final double FULL_TURN = 360.0;
	private static final double DEFAULT_ANGLE = 90.0;
	private static final int DEFAULT_ACCENT_START = 0xFF00D2FF;
	private static final int DEFAULT_ACCENT_END = 0xFFFC466B;

	private final EnumSetting<HudSurface> surface;
	private final EnumSetting<HudFill> fill;
	private final ColorSetting fillColor;
	private final DoubleSetting fillOpacity;
	private final EnumSetting<HudOutline> outline;
	private final ColorSetting outlineColor;
	private final DoubleSetting outlineWidth;
	private final EnumSetting<HudShadow> shadow;
	private final DoubleSetting shadowSize;
	private final DoubleSetting shadowStrength;
	private final EnumSetting<HudInk> text;
	private final ColorSetting textColor;
	private final BoolSetting textShadow;
	private final DoubleSetting radius;
	private final EnumSetting<AccentPalette> accent;
	private final ColorSetting accentStart;
	private final ColorSetting accentEnd;
	private final EnumSetting<HudMotion> motion;
	private final DoubleSetting speed;
	private final DoubleSetting angle;

	public HudStyle(Settings settings) {
		HudPreset preset = HudTemplate.DEFAULT.preset();
		this.surface = settings.choice("hud_surface", preset.surface());
		this.fill = settings.choice("hud_fill", preset.fill());
		this.fillColor = settings.color("hud_fill_color", preset.fillColor()).visibleWhen(() -> this.fill.is(HudFill.SOLID));
		this.fillOpacity = settings.number("hud_fill_opacity", preset.fillOpacity(), 0.0, 1.0)
			.step(0.01)
			.visibleWhen(() -> this.fill.is(HudFill.ACCENT) || this.fill.is(HudFill.THEME));
		this.outline = settings.choice("hud_outline", preset.outline());
		this.outlineColor = settings.color("hud_outline_color", preset.outlineColor()).visibleWhen(() -> this.outline.is(HudOutline.SOLID));
		this.outlineWidth = settings.number("hud_outline_width", preset.outlineWidth(), 0.5, MAX_OUTLINE_WIDTH)
			.step(0.5)
			.visibleWhen(() -> !this.outline.is(HudOutline.NONE));
		this.shadow = settings.choice("hud_shadow", preset.shadow());
		this.shadowSize = settings.number("hud_shadow_size", preset.shadowSize(), 1.0, MAX_SHADOW_SIZE)
			.step(1.0)
			.visibleWhen(() -> !this.shadow.is(HudShadow.NONE));
		this.shadowStrength = settings.number("hud_shadow_strength", preset.shadowStrength(), 0.05, 1.0)
			.step(0.05)
			.visibleWhen(() -> !this.shadow.is(HudShadow.NONE));
		this.text = settings.choice("hud_text", preset.text());
		this.textColor = settings.color("hud_text_color", preset.textColor()).visibleWhen(() -> this.text.is(HudInk.SOLID));
		this.textShadow = settings.bool("hud_text_shadow", preset.textShadow());
		this.radius = settings.number("hud_radius", preset.radius(), 0.0, MAX_RADIUS).step(0.5);
		this.accent = settings.choice("hud_accent", AccentPalette.THEME);
		this.accentStart = settings.color("hud_accent_start", DEFAULT_ACCENT_START).visibleWhen(() -> this.accent.is(AccentPalette.CUSTOM));
		this.accentEnd = settings.color("hud_accent_end", DEFAULT_ACCENT_END).visibleWhen(() -> this.accent.is(AccentPalette.CUSTOM));
		this.motion = settings.choice("hud_motion", HudMotion.STATIC);
		this.speed = settings.number("hud_speed", 1.0, MIN_SPEED, MAX_SPEED)
			.step(0.1)
			.visibleWhen(() -> !this.motion.is(HudMotion.STATIC));
		this.angle = settings.number("hud_angle", DEFAULT_ANGLE, 0.0, FULL_TURN).step(15.0);
	}

	public List<Setting<?, ?>> panelSettings() {
		return List.of(
			this.surface,
			this.fill,
			this.fillColor,
			this.fillOpacity,
			this.outline,
			this.outlineColor,
			this.outlineWidth,
			this.shadow,
			this.shadowSize,
			this.shadowStrength,
			this.radius
		);
	}

	public List<Setting<?, ?>> textSettings() {
		return List.of(this.text, this.textColor, this.textShadow);
	}

	public List<Setting<?, ?>> accentSettings() {
		return List.of(this.accentStart, this.accentEnd, this.motion, this.speed, this.angle);
	}

	public List<Setting<?, ?>> all() {
		return Stream.of(panelSettings(), textSettings(), List.<Setting<?, ?>>of(this.accent), accentSettings())
			.flatMap(List::stream)
			.toList();
	}

	public EnumSetting<HudSurface> surface() {
		return this.surface;
	}

	public EnumSetting<AccentPalette> accent() {
		return this.accent;
	}

	public boolean isMoving() {
		return !this.motion.is(HudMotion.STATIC);
	}

	public HudPreset preset() {
		return preset(this.fillColor.get().argb(), this.outlineColor.get().argb(), this.textColor.get().argb());
	}

	HudPreset shown() {
		return preset(this.fillColor.argb(), this.outlineColor.argb(), this.textColor.argb());
	}

	private HudPreset preset(int fillArgb, int outlineArgb, int textArgb) {
		return new HudPreset(
			this.surface.get(),
			this.fill.get(),
			fillArgb,
			this.fillOpacity.get(),
			this.outline.get(),
			outlineArgb,
			this.outlineWidth.get(),
			this.shadow.get(),
			this.shadowSize.get(),
			this.shadowStrength.get(),
			this.text.get(),
			textArgb,
			this.textShadow.get(),
			this.radius.get()
		);
	}

	public void apply(HudTemplate template) {
		HudPreset preset = template.preset();
		this.surface.set(preset.surface());
		this.fill.set(preset.fill());
		this.fillColor.set(new ColorValue(preset.fillColor(), false));
		this.fillOpacity.set(preset.fillOpacity());
		this.outline.set(preset.outline());
		this.outlineColor.set(new ColorValue(preset.outlineColor(), false));
		this.outlineWidth.set(preset.outlineWidth());
		this.shadow.set(preset.shadow());
		this.shadowSize.set(preset.shadowSize());
		this.shadowStrength.set(preset.shadowStrength());
		this.text.set(preset.text());
		this.textColor.set(new ColorValue(preset.textColor(), false));
		this.textShadow.set(preset.textShadow());
		this.radius.set(preset.radius());
	}

	public Optional<HudTemplate> template() {
		HudPreset worn = preset();
		for (HudTemplate template : HudTemplate.values()) {
			if (template.preset().equals(worn)) {
				return Optional.of(template);
			}
		}
		return Optional.empty();
	}

	public boolean isAnimated() {
		HudPreset worn = preset();
		boolean accented = worn.fill() == HudFill.ACCENT
			|| worn.outline() == HudOutline.ACCENT
			|| worn.shadow() == HudShadow.GLOW
			|| worn.text() == HudInk.ACCENT;
		boolean chroma = worn.fill() == HudFill.SOLID && this.fillColor.get().chroma()
			|| worn.outline() == HudOutline.SOLID && this.outlineColor.get().chroma()
			|| worn.text() == HudInk.SOLID && this.textColor.get().chroma()
			|| accented && this.accent.is(AccentPalette.CUSTOM) && (this.accentStart.get().chroma() || this.accentEnd.get().chroma());
		return accented && !this.motion.is(HudMotion.STATIC) || chroma;
	}

	ColorSetting accentStart() {
		return this.accentStart;
	}

	ColorSetting accentEnd() {
		return this.accentEnd;
	}

	HudMotion motion() {
		return this.motion.get();
	}

	double speed() {
		return this.speed.get();
	}

	double angle() {
		return this.angle.get();
	}
}
