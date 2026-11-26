package sh.rikka.client.mixin.entity;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import java.nio.ByteBuffer;
import net.minecraft.client.renderer.StagedVertexBuffer;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.render.entity.VertexCopyBatch;
import sh.rikka.client.render.skin.SkinFrame;
import sh.rikka.client.render.skin.SkinFrameHolder;
import sh.rikka.client.render.skin.SkinGroup;

@Mixin(StagedVertexBuffer.class)
abstract class StagedVertexBufferMixin implements SkinFrameHolder {
	@Unique
	private final VertexCopyBatch rikka$copies = new VertexCopyBatch();

	@Unique
	private @Nullable SkinFrame rikka$skinFrame;

	@Unique
	private @Nullable SkinGroup rikka$buildingGroup;

	@Override
	public SkinFrame rikka$skinFrame() {
		SkinFrame frame = this.rikka$skinFrame;
		if (frame == null) {
			frame = new SkinFrame(RikkaClient.skins());
			this.rikka$skinFrame = frame;
		}
		return frame;
	}

	@Override
	public @Nullable SkinFrame rikka$openSkinFrame() {
		return this.rikka$skinFrame;
	}

	@Override
	public SkinGroup rikka$buildingGroup() {
		SkinGroup group = this.rikka$buildingGroup;
		if (group == null) {
			throw new IllegalStateException("no render type group is being built");
		}
		return group;
	}

	@Override
	public void rikka$setBuildingGroup(SkinGroup group) {
		this.rikka$buildingGroup = group;
	}

	@Redirect(method = "uploadDrawsToBuffers", at = @At(value = "INVOKE", target = "Ljava/nio/ByteBuffer;put(Ljava/nio/ByteBuffer;)Ljava/nio/ByteBuffer;"))
	private ByteBuffer rikka$queueCopy(ByteBuffer target, ByteBuffer source) {
		this.rikka$copies.add(target, source);
		return target;
	}

	@WrapOperation(
		method = "uploadDrawsToBuffers",
		at = @At(value = "INVOKE", target = "Lnet/minecraft/client/renderer/StagedVertexBuffer$Draw;freeVertexData()V")
	)
	private void rikka$copyBeforeFree(StagedVertexBuffer.Draw draw, Operation<Void> original) {
		this.rikka$copies.copy();
		original.call(draw);
	}

	@Inject(method = "upload", at = @At("HEAD"))
	private void rikka$uploadSkins(CallbackInfo callback) {
		if (this.rikka$skinFrame != null) {
			this.rikka$skinFrame.upload();
		}
	}

	@Inject(method = "endDraw", at = @At("HEAD"))
	private void rikka$endSkinDraw(CallbackInfo callback) {
		this.rikka$buildingGroup = null;
		if (this.rikka$skinFrame != null) {
			this.rikka$skinFrame.endDraw();
		}
	}

	@Inject(method = "endFrame", at = @At("TAIL"))
	private void rikka$endSkinFrame(CallbackInfo callback) {
		if (this.rikka$skinFrame != null) {
			this.rikka$skinFrame.endFrame();
		}
	}

	@Inject(method = "close", at = @At("HEAD"))
	private void rikka$closeSkins(CallbackInfo callback) {
		if (this.rikka$skinFrame != null) {
			this.rikka$skinFrame.close();
			this.rikka$skinFrame = null;
		}
	}
}
