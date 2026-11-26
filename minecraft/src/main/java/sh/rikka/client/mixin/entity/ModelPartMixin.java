package sh.rikka.client.mixin.entity;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import java.util.List;
import java.util.Map;
import net.minecraft.client.model.geom.ModelPart;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Overwrite;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.render.entity.ModelPartRenderer;
import sh.rikka.client.render.entity.ModelPartTree;
import sh.rikka.client.render.skin.ModelSkin;
import sh.rikka.client.render.skin.ModelSkinHolder;

@Mixin(ModelPart.class)
abstract class ModelPartMixin implements ModelPartTree, ModelSkinHolder {
	@Unique
	private ModelPart.Cube[] rikka$cubes;

	@Unique
	private ModelPart[] rikka$children;

	@Unique
	private int rikka$boneCount;

	@Unique
	private @Nullable ModelSkin rikka$skin;

	@Unique
	private int rikka$skinGeneration;

	@Inject(method = "<init>", at = @At("RETURN"))
	private void rikka$indexTree(List<ModelPart.Cube> cubes, Map<String, ModelPart> children, CallbackInfo callback) {
		this.rikka$cubes = cubes.toArray(ModelPart.Cube[]::new);
		this.rikka$children = children.values().toArray(ModelPart[]::new);
		int boneCount = this.rikka$cubes.length == 0 ? 0 : 1;
		for (ModelPart child : this.rikka$children) {
			boneCount += ((ModelPartTree) (Object) child).rikka$boneCount();
		}
		this.rikka$boneCount = boneCount;
	}

	@Override
	public ModelPart.Cube[] rikka$cubes() {
		return this.rikka$cubes;
	}

	@Override
	public ModelPart[] rikka$children() {
		return this.rikka$children;
	}

	@Override
	public int rikka$boneCount() {
		return this.rikka$boneCount;
	}

	@Override
	public @Nullable ModelSkin rikka$skin(int generation) {
		return this.rikka$skinGeneration == generation ? this.rikka$skin : null;
	}

	@Override
	public void rikka$setSkin(ModelSkin skin, int generation) {
		this.rikka$skin = skin;
		this.rikka$skinGeneration = generation;
	}

	@Overwrite
	public void render(PoseStack poseStack, VertexConsumer buffer, int lightCoords, int overlayCoords, int color) {
		ModelPartRenderer.render((ModelPart) (Object) this, poseStack, buffer, lightCoords, overlayCoords, color);
	}
}
