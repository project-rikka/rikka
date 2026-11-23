package sh.rikka.client.menu;

import java.io.UncheckedIOException;
import java.util.List;
import java.util.Objects;
import org.jspecify.annotations.Nullable;
import os.rikka.Align;
import os.rikka.ButtonVariant;
import os.rikka.ColorRole;
import os.rikka.Context;
import os.rikka.Rikka;
import os.rikka.TextDocument;
import os.rikka.TextRole;
import os.rikka.TextTone;
import os.rikka.TransitionStyle;
import os.rikka.View;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.config.ProfileManager;

final class ProfilesPage implements AutoCloseable {
	private static final float ICON_SIZE = 22.0F;

	private final TextDocument name = new TextDocument("");
	private final Runnable switched;
	private @Nullable List<String> names;

	ProfilesPage(Runnable switched) {
		this.switched = switched;
	}

	void view(View view) {
		ProfileManager profiles = RikkaClient.profiles();
		String active = profiles.active();
		List<String> names = names(profiles);
		view.column(() -> {
				for (String profile : names) {
					row(view, profiles, profile, profile.equals(active));
				}
			})
			.gap(Rikka.SPACE_SNUG)
			.fillWidth();
		Heading.view(view, Labels.of("profiles.new"));
		String draft = this.name.getText().strip();
		boolean valid = ProfileManager.isValidName(draft) && !names.contains(draft);
		view.card(() -> view.row(() -> {
					view.textField(this.name)
						.placeholder(Labels.of("profiles.name"))
						.maxLength(ProfileManager.MAX_NAME_LENGTH)
						.invalid(!draft.isEmpty() && !valid)
						.grow(1.0F)
						.onChange(context -> {
						})
						.onSubmit(context -> create(context, profiles));
					view.button(Labels.of("profiles.create"))
						.enabled(valid)
						.onPress(context -> create(context, profiles));
				})
				.gap(Rikka.SPACE_NORMAL)
				.alignItems(Align.CENTER)
				.fillWidth());
	}

	private void row(View view, ProfileManager profiles, String profile, boolean active) {
		view.card(() -> view.row(() -> {
					view.icon(active ? "check_circle" : "person")
						.iconSize(ICON_SIZE)
						.tint(view.paletteColor(active ? ColorRole.ACCENT : ColorRole.TEXT_MUTED));
					view.column(() -> {
							view.text(profile).textStyle(TextRole.SUBHEADING);
							if (active) {
								view.text(Labels.of("profiles.active")).textStyle(TextRole.CAPTION).tone(TextTone.ACCENT);
							}
						})
						.grow(1.0F);
					if (!active) {
						view.button(Labels.of("profiles.use"))
							.buttonStyle(ButtonVariant.SECONDARY)
							.onPress(context -> use(context, profiles, profile));
						view.iconButton("delete")
							.tooltip(Labels.of("profiles.delete"))
							.onPress(context -> context.confirm(
								Labels.of("profiles.delete.title", profile),
								Labels.of("profiles.delete.message"),
								Labels.of("profiles.delete.accept"),
								(answered, confirmed) -> {
									if (confirmed) {
										delete(answered, profiles, profile);
									}
								}
							));
					}
				})
				.gap(Rikka.SPACE_NORMAL)
				.alignItems(Align.CENTER)
				.fillWidth())
			.key("profile:" + profile)
			.transition(TransitionStyle.FADE_SCALE)
			.glide();
	}

	private void create(Context context, ProfileManager profiles) {
		String draft = this.name.getText().strip();
		if (!ProfileManager.isValidName(draft) || names(profiles).contains(draft)) {
			return;
		}
		this.names = null;
		try {
			profiles.create(draft);
			this.name.reset("");
		} catch (UncheckedIOException failure) {
			failed(context, failure);
		}
	}

	private void use(Context context, ProfileManager profiles, String profile) {
		try {
			profiles.switchTo(profile);
			this.switched.run();
		} catch (UncheckedIOException failure) {
			failed(context, failure);
		}
	}

	private void delete(Context context, ProfileManager profiles, String profile) {
		this.names = null;
		try {
			profiles.delete(profile);
		} catch (UncheckedIOException failure) {
			failed(context, failure);
		}
	}

	private List<String> names(ProfileManager profiles) {
		List<String> listed = this.names;
		if (listed == null) {
			listed = profiles.profiles();
			this.names = listed;
		}
		return listed;
	}

	private static void failed(Context context, UncheckedIOException failure) {
		RikkaClient.LOGGER.error("profile operation failed", failure);
		context.alert(Labels.of("profiles.failed"), Objects.requireNonNullElse(failure.getCause().getMessage(), failure.getMessage()));
	}

	@Override
	public void close() {
		this.name.close();
	}
}
