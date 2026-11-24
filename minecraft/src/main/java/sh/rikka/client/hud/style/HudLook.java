package sh.rikka.client.hud.style;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.util.Mth;
import net.minecraft.util.Util;
import os.rikka.Color;
import os.rikka.ColorRole;
import os.rikka.Element;
import os.rikka.Gradient;
import os.rikka.View;
import sh.rikka.client.ui.style.Colors;

public final class HudLook implements AutoCloseable {
	private static final double PERIOD_MILLIS = 4000.0;
	private static final float FULL_TURN = 360.0F;
	private static final float TEXT_SHADOW_OFFSET = 1.0F;
	private static final Color TEXT_SHADOW = new Color(0.0F, 0.0F, 0.0F, 0.55F);
	private static final Color SHADOW = new Color(0.0F, 0.0F, 0.0F, 1.0F);
	private static final Color CLEAR = new Color(0.0F, 0.0F, 0.0F, 0.0F);
	private static final Color WHITE = new Color(1.0F, 1.0F, 1.0F, 1.0F);
	private static final float SHADOW_DROP = 0.2F;
	private static final float MUTED = 0.65F;
	private static final int FLOW_SEGMENTS = 4;

	private final HudPreset preset;
	private final HudMotion motion;
	private final float angle;
	private final float phase;
	private final Color accentStart;
	private final Color accentEnd;
	private final Color fillColor;
	private final Color outlineColor;
	private final Color textColor;
	private final Color themeSurface;
	private final List<Gradient> made = new ArrayList<>();
	private final Map<Float, Gradient> accents = new HashMap<>();

	private HudLook(HudStyle style, HudPreset preset, View view) {
		this.preset = preset;
		this.motion = style.motion();
		this.angle = (float) style.angle();
		double period = PERIOD_MILLIS / style.speed();
		this.phase = (float) (Util.getMillis() % (long) period / period);
		Accent accent = Accent.of(style, style.accent().get(), view);
		this.accentStart = accent.start();
		this.accentEnd = accent.end();
		this.fillColor = Colors.of(preset.fillColor());
		this.outlineColor = Colors.of(preset.outlineColor());
		this.textColor = switch (preset.text()) {
			case WHITE -> WHITE;
			case SOLID -> Colors.of(preset.textColor());
			case ACCENT -> this.accentStart;
			case THEME -> view.paletteColor(ColorRole.TEXT);
		};
		this.themeSurface = view.paletteColor(ColorRole.SURFACE);
	}

	public static HudLook of(HudStyle style, View view) {
		return new HudLook(style, style.shown(), view);
	}

	public static HudLook preview(HudStyle style, HudTemplate template, View view) {
		return new HudLook(style, template.preset(), view);
	}

	public float radius() {
		return (float) this.preset.radius();
	}

	public Color ink() {
		return this.textColor;
	}

	public Element panel(Element box) {
		return panel(box, radius());
	}

	public Element panel(Element box, float cornerRadius) {
		box.cornerRadius(cornerRadius);
		if (this.preset.surface() != HudSurface.NONE) {
			box.glass();
		}
		float fillOpacity = (float) this.preset.fillOpacity();
		switch (this.preset.fill()) {
			case NONE -> box.background(CLEAR);
			case SOLID -> box.background(this.fillColor);
			case ACCENT -> box.background(CLEAR).backgroundGradient(accent(fillOpacity));
			case THEME -> box.background(Colors.fade(this.themeSurface, fillOpacity));
		}
		float width = (float) this.preset.outlineWidth();
		switch (this.preset.outline()) {
			case NONE -> {
			}
			case SOLID -> box.border(width, this.outlineColor);
			case ACCENT -> box.borderGradient(width, accent(1.0F));
		}
		float size = (float) this.preset.shadowSize();
		float strength = (float) this.preset.shadowStrength();
		switch (this.preset.shadow()) {
			case NONE -> {
			}
			case SHADOW -> box.shadow(size, 0.0F, 0.0F, size * SHADOW_DROP, Colors.fade(SHADOW, strength));
			case GLOW -> box.shadowGradient(size, 0.0F, 0.0F, 0.0F, accent(strength));
		}
		return box;
	}

	public Element text(Element text) {
		return inked(text, 1.0F);
	}

	public Element mutedText(Element text) {
		return inked(text, MUTED);
	}

	public Element mark(Element box) {
		return marked(box, 1.0F);
	}

	public Element mutedMark(Element box) {
		return marked(box, MUTED);
	}

	private Element marked(Element box, float share) {
		if (this.preset.text() == HudInk.ACCENT) {
			return box.background(CLEAR).backgroundGradient(accent(share));
		}
		return box.background(Colors.fade(this.textColor, share));
	}

	private Element inked(Element text, float share) {
		if (this.preset.text() == HudInk.ACCENT) {
			text.tintGradient(accent(share));
		} else {
			text.tint(Colors.fade(this.textColor, share));
		}
		if (this.preset.textShadow()) {
			text.textShadow(TEXT_SHADOW_OFFSET, TEXT_SHADOW_OFFSET, Colors.fade(TEXT_SHADOW, share));
		}
		return text;
	}

	private Gradient accent(float alpha) {
		Gradient cached = this.accents.get(alpha);
		if (cached != null) {
			return cached;
		}
		Color start = Colors.fade(this.accentStart, alpha);
		Color end = Colors.fade(this.accentEnd, alpha);
		Gradient gradient = switch (this.motion) {
			case STATIC -> keep(Gradient.fromLinear(this.angle, start, end));
			case ROTATE -> keep(Gradient.fromLinear(this.angle + FULL_TURN * this.phase, start, end));
			case PULSE -> {
				float swing = (1.0F - Mth.cos(this.phase * Mth.TWO_PI)) * 0.5F;
				yield keep(Gradient.fromLinear(this.angle, Colors.mix(start, end, swing), Colors.mix(end, start, swing)));
			}
			case FLOW -> flowing(start, end);
		};
		this.accents.put(alpha, gradient);
		return gradient;
	}

	private Gradient flowing(Color start, Color end) {
		Gradient gradient = keep(Gradient.fromLinear(this.angle, wave(start, end, 0.0F), wave(start, end, 1.0F)));
		for (int segment = 1; segment < FLOW_SEGMENTS; segment++) {
			float offset = segment / (float) FLOW_SEGMENTS;
			gradient = keep(gradient.withStop(offset, wave(start, end, offset)));
		}
		return gradient;
	}

	private Color wave(Color start, Color end, float offset) {
		float travelled = Mth.positiveModulo(this.phase + offset, 1.0F);
		return Colors.mix(start, end, 1.0F - Math.abs(travelled * 2.0F - 1.0F));
	}

	private Gradient keep(Gradient gradient) {
		this.made.add(gradient);
		return gradient;
	}

	@Override
	public void close() {
		for (Gradient gradient : this.made) {
			gradient.close();
		}
		this.made.clear();
		this.accents.clear();
	}
}
