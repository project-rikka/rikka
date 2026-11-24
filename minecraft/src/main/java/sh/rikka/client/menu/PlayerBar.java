package sh.rikka.client.menu;

import java.util.Locale;
import java.util.Optional;
import os.rikka.Align;
import os.rikka.ButtonVariant;
import os.rikka.ColorRole;
import os.rikka.Element;
import os.rikka.FontWeight;
import os.rikka.Justification;
import os.rikka.Rikka;
import os.rikka.SizeClass;
import os.rikka.TextAlignment;
import os.rikka.TextRole;
import os.rikka.TextTone;
import os.rikka.View;
import sh.rikka.client.config.MusicRepeat;
import sh.rikka.client.music.MusicPlayer;
import sh.rikka.client.music.PlaybackTime;
import sh.rikka.client.music.Track;
import sh.rikka.client.ui.CoverArt;

final class PlayerBar {
	private static final float HEIGHT = 96.0F;
	private static final float SIDE_WIDTH = 240.0F;
	private static final float COVER_SIZE = 56.0F;
	private static final float TIME_WIDTH = 52.0F;
	private static final float VOLUME_WIDTH = 120.0F;
	private static final double QUIET_VOLUME = 0.5;

	private double scrubbing = Double.NaN;

	void view(View view, MusicPlayer player) {
		Optional<Track> track = player.current();
		view.row(() -> {
				nowPlaying(view, player, track);
				transport(view, player, track.isPresent());
				volume(view, player);
			})
			.gap(Rikka.SPACE_ROOMY)
			.alignItems(Align.CENTER)
			.paddingEdges(0.0F, Rikka.SPACE_LOOSE, 0.0F, Rikka.SPACE_LOOSE)
			.height(HEIGHT)
			.fillWidth();
	}

	private static void nowPlaying(View view, MusicPlayer player, Optional<Track> track) {
		view.row(() -> {
				CoverArt.view(view, track.map(Track::cover).orElse(null), COVER_SIZE, Rikka.CORNER_ITEM);
				view.column(() -> {
						if (track.isEmpty()) {
							view.text(Labels.of("music.not_playing")).tone(TextTone.MUTED).textNoWrap();
							return;
						}
						Track shown = track.get();
						view.text(shown.title()).textWeight(FontWeight.SEMIBOLD).textNoWrap();
						Optional<String> failure = player.failure(shown);
						if (failure.isPresent()) {
							view.text(Labels.of("music.failed")).textStyle(TextRole.CAPTION).tone(TextTone.DANGER).textNoWrap().tooltip(failure.get());
						} else {
							view.text(MusicPage.artist(shown)).textStyle(TextRole.CAPTION).tone(TextTone.MUTED).textNoWrap();
						}
					})
					.gap(Rikka.SPACE_HAIR)
					.grow(1.0F)
					.shrink(1.0F);
			})
			.gap(Rikka.SPACE_NORMAL)
			.alignItems(Align.CENTER)
			.width(SIDE_WIDTH)
			.shrink(0.0F);
	}

	private void transport(View view, MusicPlayer player, boolean loaded) {
		boolean playable = !player.tracks().isEmpty();
		view.column(() -> {
				view.row(() -> {
						modeButton(view, "shuffle", player.isShuffled())
							.tooltip(Labels.of(player.isShuffled() ? "music.shuffle.off" : "music.shuffle.on"))
							.enabled(playable)
							.onPress(context -> player.toggleShuffle());
						view.iconButton("skip_previous")
							.tooltip(Labels.of("music.previous"))
							.enabled(playable)
							.onPress(context -> player.previous());
						boolean playing = player.isPlaying();
						view.iconButton(playing ? "pause" : "play_arrow")
							.tooltip(Labels.of(playing ? "music.pause" : "music.play"))
							.buttonStyle(ButtonVariant.PRIMARY)
							.controlSize(SizeClass.LARGE)
							.enabled(playable)
							.onPress(context -> player.toggle());
						view.iconButton("skip_next")
							.tooltip(Labels.of("music.next"))
							.enabled(playable)
							.onPress(context -> player.next());
						MusicRepeat repeat = player.repeat();
						modeButton(view, repeat == MusicRepeat.ONE ? "repeat_one" : "repeat", repeat != MusicRepeat.OFF)
							.tooltip(Labels.of("music.repeat." + repeat.name().toLowerCase(Locale.ROOT)))
							.enabled(playable)
							.onPress(context -> player.cycleRepeat());
					})
					.gap(Rikka.SPACE_SNUG)
					.alignItems(Align.CENTER)
					.justify(Justification.CENTER)
					.fillWidth();
				seekBar(view, player, loaded);
			})
			.gap(Rikka.SPACE_TIGHT)
			.grow(1.0F)
			.shrink(1.0F);
	}

	private static Element modeButton(View view, String icon, boolean active) {
		return view.customButton(() -> view.icon(icon).tint(view.paletteColor(active ? ColorRole.ACCENT : ColorRole.TEXT_MUTED)))
			.buttonStyle(ButtonVariant.GHOST);
	}

	private void seekBar(View view, MusicPlayer player, boolean loaded) {
		double duration = player.duration();
		boolean seekable = loaded && duration > 0.0;
		double position = Double.isNaN(this.scrubbing) ? player.position() : this.scrubbing;
		view.row(() -> {
				view.text(PlaybackTime.format(position))
					.textStyle(TextRole.CAPTION)
					.tone(TextTone.MUTED)
					.textAlign(TextAlignment.RIGHT)
					.width(TIME_WIDTH);
				view.slider(seekable ? Math.min(position, duration) : 0.0)
					.range(0.0, seekable ? duration : 1.0)
					.enabled(seekable)
					.grow(1.0F)
					.onValue((context, value) -> this.scrubbing = value)
					.onValueCommit((context, value) -> {
						this.scrubbing = Double.NaN;
						player.seek(value);
					});
				view.text(PlaybackTime.format(duration))
					.textStyle(TextRole.CAPTION)
					.tone(TextTone.MUTED)
					.width(TIME_WIDTH);
			})
			.gap(Rikka.SPACE_SNUG)
			.alignItems(Align.CENTER)
			.fillWidth();
	}

	private static void volume(View view, MusicPlayer player) {
		view.row(() -> {
				view.iconButton(volumeIcon(player))
					.tooltip(Labels.of(player.isMuted() ? "music.unmute" : "music.mute"))
					.onPress(context -> player.toggleMute());
				view.slider(player.isMuted() ? 0.0 : player.volume())
					.tooltip(Labels.of("music.volume"))
					.width(VOLUME_WIDTH)
					.onValue((context, value) -> player.setVolume(value));
			})
			.gap(Rikka.SPACE_TIGHT)
			.alignItems(Align.CENTER)
			.justify(Justification.END)
			.width(SIDE_WIDTH)
			.shrink(0.0F);
	}

	private static String volumeIcon(MusicPlayer player) {
		if (player.isMuted() || player.volume() == 0.0) {
			return "volume_off";
		}
		return player.volume() < QUIET_VOLUME ? "volume_down" : "volume_up";
	}
}
