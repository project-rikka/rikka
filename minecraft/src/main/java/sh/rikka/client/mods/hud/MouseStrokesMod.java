package sh.rikka.client.mods.hud;

import os.rikka.Color;
import os.rikka.Element;
import os.rikka.View;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.LookEvent;
import sh.rikka.client.mod.HudMod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.ColorSetting;
import sh.rikka.client.setting.DoubleSetting;
import sh.rikka.client.setting.IntSetting;
import sh.rikka.client.setting.ScreenPosition;
import sh.rikka.client.ui.Smoothed;
import sh.rikka.client.ui.style.Colors;

public final class MouseStrokesMod extends HudMod {
	private static final float SIZE = 58.0F;
	private static final float DOT = 9.0F;
	private static final float INSET = 4.0F;
	private static final float RANGE = (SIZE - DOT) / 2.0F - INSET;
	private static final float GAIN = 0.2F;
	private static final float RETURN_MILLIS = 70.0F;
	private static final float REST = 0.05F;
	private static final float CORNER_RADIUS = 9.0F;
	private static final float CROSS_ALPHA = 0.18F;
	private static final float CROSS_THICKNESS = 1.0F;
	private static final int MAX_TRAIL = 16;

	private final DoubleSetting sensitivity = number("sensitivity", 1.0, 0.1, 3.0).step(0.1);
	private final BoolSetting trail = bool("trail", true);
	private final IntSetting trailLength = integer("trail_length", 8, 2, MAX_TRAIL).visibleWhen(this.trail::get);
	private final BoolSetting crosshair = bool("crosshair", true);
	private final BoolSetting background = bool("background", true);
	private final ColorSetting dotColor = color("dot_color", 0xFFFFFFFF);
	private final float[] trailX = new float[MAX_TRAIL];
	private final float[] trailY = new float[MAX_TRAIL];
	private final Smoothed offsetX = new Smoothed(RETURN_MILLIS, REST, 0.0F);
	private final Smoothed offsetY = new Smoothed(RETURN_MILLIS, REST, 0.0F);
	private int trailStart;
	private int trailCount;
	private double pendingX;
	private double pendingY;

	public MouseStrokesMod(ScreenPosition defaultPosition) {
		super("mouse_strokes", defaultPosition, "trackpad_input");
	}

	@Override
	protected void onDisable() {
		this.offsetX.snap(0.0F);
		this.offsetY.snap(0.0F);
		this.pendingX = 0.0;
		this.pendingY = 0.0;
		this.trailCount = 0;
	}

	@EventHandler
	private void onLook(LookEvent event) {
		this.pendingX += event.horizontal();
		this.pendingY += event.vertical();
	}

	@Override
	public float width() {
		return SIZE;
	}

	@Override
	public float height() {
		return SIZE;
	}

	@Override
	public boolean animating() {
		return this.pendingX != 0.0 || this.pendingY != 0.0 || this.offsetX.value() != 0.0F || this.offsetY.value() != 0.0F || this.trailCount > 1;
	}

	@Override
	protected void view(View view) {
		step();
		Element panel = view.box(() -> {
		}).position(0.0F, 0.0F).width(SIZE).height(SIZE).cornerRadius(CORNER_RADIUS);
		if (this.background.get()) {
			panel.glass().background(BACKDROP);
		}
		Color dot = Colors.of(this.dotColor.argb());
		if (this.crosshair.get()) {
			Color line = Colors.withAlpha(dot, dot.a() * CROSS_ALPHA);
			view.box(() -> {
			}).position(SIZE / 2.0F - CROSS_THICKNESS / 2.0F, INSET).width(CROSS_THICKNESS).height(SIZE - INSET * 2.0F).background(line);
			view.box(() -> {
			}).position(INSET, SIZE / 2.0F - CROSS_THICKNESS / 2.0F).width(SIZE - INSET * 2.0F).height(CROSS_THICKNESS).background(line);
		}
		if (this.trail.get()) {
			for (int i = 0; i < this.trailCount - 1; i++) {
				int index = (this.trailStart + i) % MAX_TRAIL;
				float age = (i + 1.0F) / this.trailCount;
				float size = DOT * (0.35F + 0.5F * age);
				dot(view, this.trailX[index], this.trailY[index], size, Colors.withAlpha(dot, dot.a() * age * 0.5F));
			}
		}
		dot(view, this.offsetX.value(), this.offsetY.value(), DOT, dot);
	}

	private void step() {
		float gain = GAIN * this.sensitivity.getFloat();
		this.offsetX.setValue(clamp(this.offsetX.value() + (float) this.pendingX * gain));
		this.offsetY.setValue(clamp(this.offsetY.value() + (float) this.pendingY * gain));
		this.pendingX = 0.0;
		this.pendingY = 0.0;
		this.offsetX.update(0.0F);
		this.offsetY.update(0.0F);
		recordTrail();
	}

	private void recordTrail() {
		int length = this.trail.get() ? this.trailLength.get() : 0;
		boolean resting = this.offsetX.value() == 0.0F && this.offsetY.value() == 0.0F;
		if (resting && this.trailCount > 0) {
			this.trailStart = (this.trailStart + 1) % MAX_TRAIL;
			this.trailCount--;
			return;
		}
		if (length == 0 || resting) {
			this.trailCount = 0;
			return;
		}
		int index = (this.trailStart + this.trailCount) % MAX_TRAIL;
		this.trailX[index] = this.offsetX.value();
		this.trailY[index] = this.offsetY.value();
		if (this.trailCount < length) {
			this.trailCount++;
		} else {
			this.trailStart = (this.trailStart + 1) % MAX_TRAIL;
		}
		while (this.trailCount > length) {
			this.trailStart = (this.trailStart + 1) % MAX_TRAIL;
			this.trailCount--;
		}
	}

	private static void dot(View view, float x, float y, float size, Color color) {
		view.box(() -> {
		}).position(SIZE / 2.0F + x - size / 2.0F, SIZE / 2.0F + y - size / 2.0F).width(size).height(size).cornerRadius(size / 2.0F).background(color);
	}

	private static float clamp(float value) {
		return Math.max(-RANGE, Math.min(RANGE, value));
	}
}
