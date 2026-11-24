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
import sh.rikka.client.config.ServerPattern;

final class ProfilesPage implements AutoCloseable {
	private static final float ICON_SIZE = 22.0F;

	private final TextDocument name = new TextDocument("");
	private final TextDocument server = new TextDocument("");
	private final ServerEditor servers = new ServerEditor();
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
		boolean nameValid = ProfileManager.isValidName(draft) && !names.contains(draft);
		ServerDraft serverDraft = ServerDraft.of(profiles, this.server.getText());
		boolean valid = nameValid && !(serverDraft instanceof ServerDraft.Rejected);
		view.card(() -> view.column(() -> {
						view.row(() -> {
								view.textField(this.name)
									.placeholder(Labels.of("profiles.name"))
									.maxLength(ProfileManager.MAX_NAME_LENGTH)
									.invalid(!draft.isEmpty() && !nameValid)
									.grow(1.0F)
									.onChange(context -> {
									})
									.onSubmit(context -> create(context, profiles));
								view.textField(this.server)
									.placeholder(Labels.of("profiles.server.optional"))
									.invalid(serverDraft instanceof ServerDraft.Rejected)
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
							.fillWidth();
						serverDraft.viewProblem(view);
					})
					.gap(Rikka.SPACE_SNUG)
					.fillWidth());
	}

	static void failed(Context context, UncheckedIOException failure) {
		RikkaClient.LOGGER.error("profile operation failed", failure);
		context.alert(Labels.of("profiles.failed"), Objects.requireNonNullElse(failure.getCause().getMessage(), failure.getMessage()));
	}

	private void row(View view, ProfileManager profiles, String profile, boolean active) {
		boolean editing = this.servers.isEditing(profile);
		view.card(() -> view.column(() -> {
						view.row(() -> {
								view.icon(active ? "check_circle" : "person")
									.iconSize(ICON_SIZE)
									.tint(view.paletteColor(active ? ColorRole.ACCENT : ColorRole.TEXT_MUTED));
								view.column(() -> {
										view.text(profile).textStyle(TextRole.SUBHEADING);
										if (active) {
											String badge = profiles.isOverridden() ? "profiles.active.server" : "profiles.active";
											view.text(Labels.of(badge)).textStyle(TextRole.CAPTION).tone(TextTone.ACCENT);
										}
									})
									.grow(1.0F);
								view.customButton(() -> view.icon("dns").tint(view.paletteColor(editing ? ColorRole.ACCENT : ColorRole.TEXT_MUTED)))
									.buttonStyle(ButtonVariant.GHOST)
									.tooltip(Labels.of("profiles.servers"))
									.onPress(context -> this.servers.toggle(profile));
								if (!active) {
									view.button(Labels.of("profiles.use"))
										.buttonStyle(ButtonVariant.SECONDARY)
										.onPress(context -> use(context, profiles, profile));
								}
								if (!active && !profile.equals(profiles.selected())) {
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
							.fillWidth();
						this.servers.view(view, profiles, profile);
					})
					.gap(Rikka.SPACE_SNUG)
					.fillWidth())
			.key("profile:" + profile)
			.transition(TransitionStyle.FADE_SCALE)
			.glide();
	}

	private void create(Context context, ProfileManager profiles) {
		String draft = this.name.getText().strip();
		ServerDraft serverDraft = ServerDraft.of(profiles, this.server.getText());
		if (!ProfileManager.isValidName(draft) || names(profiles).contains(draft) || serverDraft instanceof ServerDraft.Rejected) {
			return;
		}
		List<ServerPattern> patterns = serverDraft instanceof ServerDraft.Ready ready ? List.of(ready.pattern()) : List.of();
		this.names = null;
		try {
			profiles.create(draft, patterns);
			this.name.reset("");
			this.server.reset("");
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
		this.servers.forget(profile);
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

	@Override
	public void close() {
		this.name.close();
		this.server.close();
		this.servers.close();
	}
}
