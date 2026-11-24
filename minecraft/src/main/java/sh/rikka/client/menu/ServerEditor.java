package sh.rikka.client.menu;

import java.io.UncheckedIOException;
import java.util.ArrayList;
import java.util.List;
import org.jspecify.annotations.Nullable;
import os.rikka.Align;
import os.rikka.ColorRole;
import os.rikka.Context;
import os.rikka.Rikka;
import os.rikka.SizeClass;
import os.rikka.TextDocument;
import os.rikka.TransitionStyle;
import os.rikka.View;
import sh.rikka.client.config.ProfileManager;
import sh.rikka.client.config.ServerPattern;

final class ServerEditor implements AutoCloseable {
	private static final float CHIP_ICON_SIZE = 14.0F;

	private final TextDocument address = new TextDocument("");
	private @Nullable String editing;

	boolean isEditing(String profile) {
		return profile.equals(this.editing);
	}

	void toggle(String profile) {
		this.editing = isEditing(profile) ? null : profile;
		this.address.reset("");
	}

	void forget(String profile) {
		if (isEditing(profile)) {
			toggle(profile);
		}
	}

	void view(View view, ProfileManager profiles, String profile) {
		List<ServerPattern> servers = profiles.servers(profile);
		boolean editing = isEditing(profile);
		if (!servers.isEmpty()) {
			view.row(() -> {
					for (ServerPattern server : servers) {
						chip(view, profiles, profile, server, editing);
					}
				})
				.wrapItems()
				.gap(Rikka.SPACE_SNUG)
				.fillWidth();
		}
		if (!editing) {
			return;
		}
		ServerDraft draft = ServerDraft.of(profiles, this.address.getText());
		view.row(() -> {
				view.textField(this.address)
					.placeholder(Labels.of("profiles.server.address"))
					.invalid(draft instanceof ServerDraft.Rejected)
					.autofocus(true)
					.grow(1.0F)
					.onChange(context -> {
					})
					.onSubmit(context -> add(context, profiles, profile));
				view.button(Labels.of("list.add"))
					.enabled(draft instanceof ServerDraft.Ready)
					.onPress(context -> add(context, profiles, profile));
			})
			.gap(Rikka.SPACE_NORMAL)
			.alignItems(Align.CENTER)
			.fillWidth();
		draft.viewProblem(view);
	}

	private void chip(View view, ProfileManager profiles, String profile, ServerPattern server, boolean editing) {
		view.box(() -> view.row(() -> {
						view.icon("dns").iconSize(CHIP_ICON_SIZE).tint(view.paletteColor(ColorRole.TEXT_MUTED));
						view.text(server.toString()).textNoWrap();
						if (editing) {
							view.iconButton("close")
								.controlSize(SizeClass.SMALL)
								.tooltip(Labels.of("list.remove"))
								.onPress(context -> remove(context, profiles, profile, server));
						}
					})
					.gap(Rikka.SPACE_TIGHT)
					.alignItems(Align.CENTER))
			.paddingEdges(Rikka.SPACE_TIGHT, Rikka.SPACE_SNUG, Rikka.SPACE_TIGHT, Rikka.SPACE_SNUG)
			.background(view.paletteColor(ColorRole.SURFACE_VARIANT))
			.cornerRadius(Rikka.CORNER_ITEM)
			.key("server:" + profile + ":" + server)
			.transition(TransitionStyle.FADE_SCALE)
			.glide();
	}

	private void add(Context context, ProfileManager profiles, String profile) {
		if (!(ServerDraft.of(profiles, this.address.getText()) instanceof ServerDraft.Ready ready)) {
			return;
		}
		List<ServerPattern> servers = new ArrayList<>(profiles.servers(profile));
		servers.add(ready.pattern());
		if (save(context, profiles, profile, servers)) {
			this.address.reset("");
		}
	}

	private void remove(Context context, ProfileManager profiles, String profile, ServerPattern server) {
		List<ServerPattern> servers = new ArrayList<>(profiles.servers(profile));
		servers.remove(server);
		save(context, profiles, profile, servers);
	}

	private static boolean save(Context context, ProfileManager profiles, String profile, List<ServerPattern> servers) {
		try {
			profiles.setServers(profile, servers);
			return true;
		} catch (UncheckedIOException failure) {
			ProfilesPage.failed(context, failure);
			return false;
		}
	}

	@Override
	public void close() {
		this.address.close();
	}
}
