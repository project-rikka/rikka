package sh.rikka.client.mods.subtitles;

import com.mojang.blaze3d.audio.ListenerTransform;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.resources.language.I18n;
import net.minecraft.util.Util;
import os.rikka.Align;
import os.rikka.Element;
import os.rikka.Justification;
import os.rikka.TransitionStyle;
import os.rikka.View;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.TickEvent;
import sh.rikka.client.hud.style.HudLook;
import sh.rikka.client.mod.HudMod;
import sh.rikka.client.setting.Anchor;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.DoubleSetting;
import sh.rikka.client.setting.IntSetting;
import sh.rikka.client.setting.ScreenPosition;

public final class SoundSubtitlesMod extends HudMod {
	private static final float ROW_HEIGHT = 22.0F;
	private static final float ROW_GAP = 3.0F;
	private static final float PADDING = 7.0F;
	private static final float CONTENT_GAP = 6.0F;
	private static final float ICON_SIZE = 13.0F;
	private static final float TEXT_SIZE = 12.0F;
	private static final float DETAIL_SIZE = 10.0F;
	private static final float FADED_OPACITY = 0.35F;
	private static final double MILLIS_PER_SECOND = 1000.0;
	private static final List<Line> SAMPLES = List.of(
		new Line("sample:0", "subtitles.entity.zombie.ambient", Bearing.LEFT, 1, 12.0, 1.0F),
		new Line("sample:1", "subtitles.block.chest.open", Bearing.AHEAD_RIGHT, 2, 4.0, 0.8F),
		new Line("sample:2", "subtitles.entity.creeper.primed", Bearing.BEHIND, 1, 3.0, 0.6F)
	);

	private final IntSetting rows = integer("rows", 5, 1, 10);
	private final DoubleSetting duration = number("duration", 3.0, 1.0, 10.0).step(0.5);
	private final IntSetting width = integer("width", 220, 120, 360).step(10);
	private final BoolSetting direction = bool("direction", true);
	private final BoolSetting plays = bool("plays", true);
	private final BoolSetting distance = bool("distance", false);
	private final BoolSetting background = bool("background", true);
	private final SubtitleLog log = new SubtitleLog(this.rows::get);
	private boolean listening;

	public SoundSubtitlesMod(ScreenPosition defaultPosition) {
		super("sound_subtitles", defaultPosition, "subtitles");
	}

	@Override
	protected void onDisable() {
		if (this.listening) {
			Minecraft.getInstance().getSoundManager().removeListener(this.log);
			this.listening = false;
		}
		this.log.clear();
	}

	@EventHandler
	private void onTick(TickEvent.Post event) {
		if (!this.listening) {
			Minecraft.getInstance().getSoundManager().addListener(this.log);
			this.listening = true;
		}
	}

	@Override
	public float width() {
		return this.width.get();
	}

	@Override
	public float height() {
		int count = this.rows.get();
		return count * ROW_HEIGHT + (count - 1) * ROW_GAP;
	}

	@Override
	public boolean animating() {
		return !this.log.isEmpty();
	}

	@Override
	protected void view(View view, HudLook look) {
		List<Line> lines = lines();
		if (lines.isEmpty()) {
			return;
		}
		Anchor anchor = position().get().anchor();
		boolean stackUp = anchor.vertical() >= 0.5F;
		Align align = anchor.horizontal() < 0.5F ? Align.START : anchor.horizontal() > 0.5F ? Align.END : Align.CENTER;
		TransitionStyle entrance = switch (align) {
			case START -> TransitionStyle.SLIDE_FROM_LEADING;
			case END -> TransitionStyle.SLIDE_FROM_TRAILING;
			case CENTER, STRETCH -> TransitionStyle.FADE_SCALE;
		};
		view.column(() -> {
				for (int i = 0; i < lines.size(); i++) {
					row(view, look, lines.get(stackUp ? i : lines.size() - 1 - i), entrance);
				}
			})
			.fillWidth()
			.fillHeight()
			.gap(ROW_GAP)
			.alignItems(align)
			.justify(stackUp ? Justification.END : Justification.START);
	}

	private List<Line> lines() {
		Minecraft minecraft = Minecraft.getInstance();
		ListenerTransform listener = minecraft.getSoundManager().getListenerTransform();
		long now = Util.getMillis();
		long durationMillis = Math.round(this.duration.get() * MILLIS_PER_SECOND);
		List<Subtitle> heard = this.log.shown(listener.position(), now, durationMillis);
		if (heard.isEmpty()) {
			return editing() ? SAMPLES.stream().map(Line::translated).toList() : List.of();
		}
		List<Line> lines = new ArrayList<>(heard.size());
		for (Subtitle subtitle : heard) {
			Bearing bearing = subtitle.relative() ? Bearing.AROUND : Bearing.of(listener, subtitle.location());
			double metres = subtitle.relative() ? 0.0 : subtitle.location().distanceTo(listener.position());
			float opacity = FADED_OPACITY + (1.0F - FADED_OPACITY) * subtitle.freshness(now, durationMillis);
			lines.add(new Line("subtitle:" + subtitle.id(), subtitle.text(), bearing, subtitle.plays(), metres, opacity));
		}
		return lines;
	}

	private void row(View view, HudLook look, Line line, TransitionStyle entrance) {
		Element pill = view.box(() -> view.row(() -> content(view, look, line))
				.fillHeight()
				.gap(CONTENT_GAP)
				.alignItems(Align.CENTER))
			.height(ROW_HEIGHT)
			.maxWidth(width())
			.shrink(0.0F)
			.paddingEdges(0.0F, PADDING, 0.0F, PADDING)
			.opacity(line.opacity())
			.key(line.key())
			.transition(entrance)
			.glide();
		if (this.background.get()) {
			look.panel(pill);
		}
	}

	private void content(View view, HudLook look, Line line) {
		if (this.direction.get()) {
			look.text(view.icon(line.bearing().icon()).iconSize(ICON_SIZE));
		}
		look.text(view.text(line.text()).textSize(TEXT_SIZE).textNoWrap()).shrink(1.0F);
		if (this.plays.get() && line.plays() > 1) {
			look.mutedText(view.text(I18n.get(translationKey() + ".plays", line.plays())).textSize(DETAIL_SIZE).textNoWrap());
		}
		if (this.distance.get() && line.bearing() != Bearing.AROUND) {
			look.mutedText(view.text(I18n.get(translationKey() + ".distance", Math.round(line.metres()))).textSize(DETAIL_SIZE).textNoWrap());
		}
	}

	private record Line(String key, String text, Bearing bearing, int plays, double metres, float opacity) {
		Line translated() {
			return new Line(this.key, I18n.get(this.text), this.bearing, this.plays, this.metres, this.opacity);
		}
	}
}
