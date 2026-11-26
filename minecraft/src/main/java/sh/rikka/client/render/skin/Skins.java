package sh.rikka.client.render.skin;

import it.unimi.dsi.fastutil.objects.Reference2ReferenceOpenHashMap;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.ResourceReloadEvent;
import sh.rikka.client.mods.boost.BoostFeature;

public final class Skins implements AutoCloseable {
	private final SkinMeshPool meshes = new SkinMeshPool();
	private final SkinPipelines pipelines = new SkinPipelines();
	private final Reference2ReferenceOpenHashMap<List<BakedQuad>, ItemSkin> items = new Reference2ReferenceOpenHashMap<>();
	private int generation = 1;
	private boolean loaded;

	public Skins() {
		BoostFeature.GPU_ENTITIES.onChange(this::compile);
	}

	public boolean isActive() {
		return BoostFeature.GPU_ENTITIES.isActive() && this.loaded;
	}

	public SkinPipelines pipelines() {
		return this.pipelines;
	}

	SkinMeshPool meshes() {
		return this.meshes;
	}

	public ModelSkin model(ModelPart root) {
		ModelSkinHolder holder = (ModelSkinHolder) (Object) root;
		ModelSkin skin = holder.rikka$skin(this.generation);
		if (skin == null) {
			skin = ModelSkin.build(root, this.meshes);
			holder.rikka$setSkin(skin, this.generation);
		}
		return skin;
	}

	public ItemSkin item(List<BakedQuad> quads) {
		ItemSkin skin = this.items.get(quads);
		if (skin == null) {
			skin = ItemSkin.build(quads, this.meshes, this.pipelines);
			this.items.put(quads, skin);
		}
		return skin;
	}

	@EventHandler
	private void onResourceReload(ResourceReloadEvent event) {
		this.generation++;
		this.meshes.clear();
		this.items.clear();
		this.pipelines.reload(Minecraft.getInstance().getResourceManager());
		this.loaded = true;
		compile();
	}

	private void compile() {
		if (isActive() && !this.pipelines.compileAll()) {
			this.items.clear();
		}
	}

	@Override
	public void close() {
		this.meshes.close();
	}
}
