package sh.rikka.client.mods.hud;

import java.util.Optional;
import net.minecraft.client.resources.language.I18n;
import os.rikka.Align;
import os.rikka.Element;
import os.rikka.FontWeight;
import os.rikka.Justification;
import os.rikka.View;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.hud.style.HudLook;
import sh.rikka.client.mod.HudMod;
import sh.rikka.client.music.MusicPlayer;
import sh.rikka.client.music.Track;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.ScreenPosition;
import sh.rikka.client.ui.CoverArt;

public final class MusicInfoMod extends HudMod {
	private static final float WIDTH = 230.0F;
	private static final float HEIGHT = 58.0F;
	private static final float PADDING = 7.0F;
	private static final float GAP = 8.0F;
	private static final float COVER_SIZE = HEIGHT - PADDING * 2.0F;
	private static final float COVER_RADIUS = 5.0F;
	private static final float TITLE_SIZE = 13.0F;
	private static final float ARTIST_SIZE = 11.0F;
	private static final float PROGRESS_HEIGHT = 3.0F;
	private static final float TEXT_GAP = 3.0F;

	private final BoolSetting cover = bool("cover", true);
	private final BoolSetting progress = bool("progress", true);
	private final BoolSetting background = bool("background", true);

	public MusicInfoMod(ScreenPosition defaultPosition) {
		super("music_info", defaultPosition, "music_note");
	}

	@Override
	public float width() {
		return WIDTH;
	}

	@Override
	public float height() {
		return HEIGHT;
	}

	@Override
	protected void view(View view, HudLook look) {
		MusicPlayer player = RikkaClient.music();
		Optional<Track> track = player.current();
		if (track.isEmpty() && !editing()) {
			return;
		}
		Element panel = view.box(() -> view.row(() -> {
					if (this.cover.get()) {
						CoverArt.view(view, track.map(Track::cover).orElse(null), COVER_SIZE, COVER_RADIUS);
					}
					view.column(() -> details(view, look, player, track))
						.gap(TEXT_GAP)
						.justify(Justification.CENTER)
						.grow(1.0F)
						.shrink(1.0F)
						.fillHeight();
				})
				.gap(GAP)
				.alignItems(Align.CENTER)
				.fillWidth()
				.fillHeight())
			.fillWidth()
			.fillHeight()
			.padding(PADDING);
		if (this.background.get()) {
			look.panel(panel);
		}
	}

	private void details(View view, HudLook look, MusicPlayer player, Optional<Track> track) {
		String title = track.map(Track::title).orElseGet(() -> I18n.get(translationKey() + ".not_playing"));
		look.text(view.text(title).textSize(TITLE_SIZE).textWeight(FontWeight.MEDIUM).textNoWrap());
		track.filter(shown -> !shown.artist().isEmpty())
			.ifPresent(shown -> look.mutedText(view.text(shown.artist()).textSize(ARTIST_SIZE).textNoWrap()));
		if (this.progress.get()) {
			double duration = player.duration();
			view.progress(duration > 0.0 ? Math.clamp(player.position() / duration, 0.0, 1.0) : 0.0)
				.height(PROGRESS_HEIGHT)
				.tint(look.ink())
				.fillWidth();
		}
	}
}
