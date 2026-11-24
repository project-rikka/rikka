package sh.rikka.client.menu;

import java.util.List;
import java.util.Optional;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import os.rikka.Align;
import os.rikka.Color;
import os.rikka.ColorRole;
import os.rikka.Element;
import os.rikka.FontWeight;
import os.rikka.Justification;
import os.rikka.Rikka;
import os.rikka.RikkaException;
import os.rikka.TextAlignment;
import os.rikka.TextRole;
import os.rikka.TextTone;
import os.rikka.TransitionStyle;
import os.rikka.View;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.music.MusicPlayer;
import sh.rikka.client.music.PlaybackTime;
import sh.rikka.client.music.Track;
import sh.rikka.client.ui.CoverArt;
import sh.rikka.client.ui.style.Colors;

final class MusicPage {
	private static final Logger LOGGER = LoggerFactory.getLogger(MusicPage.class);
	private static final float ROW_HEIGHT = 56.0F;
	private static final float ROW_GAP = 4.0F;
	private static final float COVER_SIZE = 40.0F;
	private static final float MARKER_WIDTH = 24.0F;
	private static final float ALBUM_SHARE = 0.6F;
	private static final float DURATION_WIDTH = 56.0F;
	private static final float CURRENT_ALPHA = 0.14F;
	private static final float FAILED_OPACITY = 0.5F;

	private final PlayerBar bar = new PlayerBar();

	void view(View view, MenuState state) {
		MusicPlayer player = RikkaClient.music();
		view.column(() -> {
				library(view, state, player);
				view.divider();
				this.bar.view(view, player);
			})
			.fillWidth()
			.fillHeight();
	}

	static void openFolder() {
		try {
			Rikka.openFile(RikkaClient.music().folder().toString());
		} catch (RikkaException failure) {
			LOGGER.error("failed to open the music folder", failure);
		}
	}

	static String artist(Track track) {
		return track.artist().isEmpty() ? Labels.of("music.unknown_artist") : track.artist();
	}

	private static void library(View view, MenuState state, MusicPlayer player) {
		if (!player.isLoaded()) {
			view.column(view::spinner)
				.grow(1.0F)
				.fillWidth()
				.alignItems(Align.CENTER)
				.justify(Justification.CENTER)
				.key("music:loading");
			return;
		}
		if (player.tracks().isEmpty()) {
			view.statusPage("library_music", Labels.of("music.empty"), () -> view.button(Labels.of("music.open_folder"))
					.onPress(context -> openFolder()))
				.grow(1.0F)
				.fillWidth()
				.key("music:empty")
				.transition(TransitionStyle.FADE);
			return;
		}
		List<Track> shown = state.shownTracks(player.tracks());
		if (shown.isEmpty()) {
			view.statusPage("search_off", Labels.of("music.no_results"), () -> {
				})
				.grow(1.0F)
				.fillWidth()
				.key("music:no_results")
				.transition(TransitionStyle.FADE);
			return;
		}
		view.lazyColumn(shown.size(), (item, index, selected) -> row(item, player, shown.get(index)))
			.itemExtent(ROW_HEIGHT)
			.gap(ROW_GAP)
			.paddingEdges(ROW_GAP, Rikka.SPACE_SNUG, ROW_GAP, Rikka.SPACE_SNUG)
			.grow(1.0F)
			.fillWidth()
			.key("music:tracks");
	}

	private static void row(View view, MusicPlayer player, Track track) {
		boolean current = player.isCurrent(track);
		Optional<String> failure = player.failure(track);
		Color accent = view.paletteColor(ColorRole.ACCENT);
		Element row = view.box(() -> view.row(() -> {
					marker(view, player, current, accent);
					CoverArt.view(view, track.cover(), COVER_SIZE, Rikka.CORNER_DETAIL);
					view.column(() -> {
							Element title = view.text(track.title()).textWeight(FontWeight.SEMIBOLD).textNoWrap();
							if (current) {
								title.tint(accent);
							}
							view.text(artist(track)).textStyle(TextRole.CAPTION).tone(TextTone.MUTED).textNoWrap();
						})
						.gap(Rikka.SPACE_HAIR)
						.grow(1.0F)
						.shrink(1.0F);
					view.text(track.album())
						.textStyle(TextRole.CAPTION)
						.tone(TextTone.MUTED)
						.textNoWrap()
						.grow(ALBUM_SHARE)
						.shrink(1.0F);
					view.text(track.duration() > 0.0 ? PlaybackTime.format(track.duration()) : "")
						.textStyle(TextRole.CAPTION)
						.tone(TextTone.MUTED)
						.textAlign(TextAlignment.RIGHT)
						.width(DURATION_WIDTH);
				})
				.gap(Rikka.SPACE_NORMAL)
				.alignItems(Align.CENTER)
				.fillWidth()
				.fillHeight())
			.paddingEdges(0.0F, Rikka.SPACE_NORMAL, 0.0F, Rikka.SPACE_SNUG)
			.height(ROW_HEIGHT)
			.fillWidth()
			.cornerRadius(Rikka.CORNER_ITEM)
			.background(Colors.withAlpha(accent, current ? CURRENT_ALPHA : 0.0F))
			.opacity(failure.isPresent() ? FAILED_OPACITY : 1.0F)
			.key("track:" + track.key())
			.onPress(context -> player.play(track));
		failure.ifPresent(row::tooltip);
	}

	private static void marker(View view, MusicPlayer player, boolean current, Color accent) {
		view.column(() -> {
				if (current) {
					view.icon(player.isPlaying() ? "graphic_eq" : "music_note").tint(accent);
				}
			})
			.width(MARKER_WIDTH)
			.alignItems(Align.CENTER)
			.shrink(0.0F);
	}
}
