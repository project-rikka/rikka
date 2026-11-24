package sh.rikka.client.hud.style;

public record HudPreset(
	HudSurface surface,
	HudFill fill,
	int fillColor,
	double fillOpacity,
	HudOutline outline,
	int outlineColor,
	double outlineWidth,
	HudShadow shadow,
	double shadowSize,
	double shadowStrength,
	HudInk text,
	int textColor,
	boolean textShadow,
	double radius
) {
	static Builder builder() {
		return new Builder();
	}

	static final class Builder {
		private static final int CLEAR_BLACK = 0x66000000;
		private static final int WHITE = 0xFFFFFFFF;
		private static final double OPACITY = 0.85;
		private static final double OUTLINE_WIDTH = 1.5;
		private static final double SHADOW_SIZE = 12.0;
		private static final double SHADOW_STRENGTH = 0.35;
		private static final double ROUNDED = 9.0;

		private HudSurface surface = HudSurface.NONE;
		private HudFill fill = HudFill.NONE;
		private int fillColor = CLEAR_BLACK;
		private double fillOpacity = OPACITY;
		private HudOutline outline = HudOutline.NONE;
		private int outlineColor = WHITE;
		private double outlineWidth = OUTLINE_WIDTH;
		private HudShadow shadow = HudShadow.NONE;
		private double shadowSize = SHADOW_SIZE;
		private double shadowStrength = SHADOW_STRENGTH;
		private HudInk text = HudInk.WHITE;
		private int textColor = WHITE;
		private boolean textShadow;
		private double radius = ROUNDED;

		private Builder() {
		}

		Builder surface(HudSurface surface) {
			this.surface = surface;
			return this;
		}

		Builder fill(int argb) {
			this.fill = HudFill.SOLID;
			this.fillColor = argb;
			return this;
		}

		Builder fill(HudFill fill, double opacity) {
			this.fill = fill;
			this.fillOpacity = opacity;
			return this;
		}

		Builder outline(int argb, double width) {
			this.outline = HudOutline.SOLID;
			this.outlineColor = argb;
			this.outlineWidth = width;
			return this;
		}

		Builder accentOutline() {
			this.outline = HudOutline.ACCENT;
			return this;
		}

		Builder shadow(HudShadow shadow, double strength) {
			this.shadow = shadow;
			this.shadowStrength = strength;
			return this;
		}

		Builder text(HudInk text) {
			this.text = text;
			return this;
		}

		Builder textShadow() {
			this.textShadow = true;
			return this;
		}

		Builder radius(double radius) {
			this.radius = radius;
			return this;
		}

		HudPreset build() {
			return new HudPreset(
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
				this.text,
				this.textColor,
				this.textShadow,
				this.radius
			);
		}
	}
}
