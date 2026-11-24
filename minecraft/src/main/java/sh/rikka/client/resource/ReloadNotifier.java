package sh.rikka.client.resource;

import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.server.packs.resources.ResourceManagerReloadListener;
import sh.rikka.client.events.ClientHooks;

public final class ReloadNotifier implements ResourceManagerReloadListener {
	@Override
	public void onResourceManagerReload(ResourceManager resourceManager) {
		ClientHooks.resourcesReloaded();
	}
}
