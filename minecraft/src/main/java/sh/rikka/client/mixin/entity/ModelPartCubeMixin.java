package sh.rikka.client.mixin.entity;

import net.minecraft.client.model.geom.ModelPart;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.render.entity.CubeMesh;
import sh.rikka.client.render.entity.CubeMeshHolder;

@Mixin(ModelPart.Cube.class)
abstract class ModelPartCubeMixin implements CubeMeshHolder {
	@Shadow
	@Final
	public ModelPart.Polygon[] polygons;

	@Unique
	private CubeMesh rikka$mesh;

	@Inject(method = "<init>", at = @At("RETURN"))
	private void rikka$buildMesh(CallbackInfo callback) {
		this.rikka$mesh = CubeMesh.of(this.polygons);
	}

	@Override
	public CubeMesh rikka$mesh() {
		return this.rikka$mesh;
	}
}
