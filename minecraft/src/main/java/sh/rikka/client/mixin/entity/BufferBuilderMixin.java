package sh.rikka.client.mixin.entity;

import com.mojang.blaze3d.vertex.BufferBuilder;
import com.mojang.blaze3d.vertex.ByteBufferBuilder;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.QuadInstance;
import com.mojang.blaze3d.vertex.VertexConsumer;
import com.mojang.renderpearl.api.pipeline.PrimitiveTopology;
import com.mojang.renderpearl.api.vertex.VertexFormat;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import sh.rikka.client.render.entity.BakedQuadWriter;
import sh.rikka.client.render.entity.QuadVertexSink;

@Mixin(BufferBuilder.class)
abstract class BufferBuilderMixin implements VertexConsumer, QuadVertexSink {
	@Unique
	private static final int MAX_VERTEX_COUNT = 16777215;

	@Shadow
	@Final
	private ByteBufferBuilder buffer;

	@Shadow
	private long vertexPointer;

	@Shadow
	private int vertices;

	@Shadow
	@Final
	private PrimitiveTopology primitiveTopology;

	@Shadow
	@Final
	private VertexFormat format;

	@Shadow
	@Final
	private int vertexSize;

	@Shadow
	private int elementsToFill;

	@Shadow
	private boolean building;

	@Override
	public boolean rikka$takesQuads(VertexFormat format) {
		return this.format == format && this.primitiveTopology == PrimitiveTopology.QUADS;
	}

	@Override
	public long rikka$reserveVertices(int count) {
		if (!this.building || this.elementsToFill != 0 || this.vertices > MAX_VERTEX_COUNT - count) {
			return NO_RESERVATION;
		}
		long pointer = this.buffer.reserve(count * this.vertexSize);
		this.vertices += count;
		this.vertexPointer = pointer + (long) (count - 1) * this.vertexSize;
		return pointer;
	}

	@Override
	public void putBakedQuad(PoseStack.Pose pose, BakedQuad quad, QuadInstance instance) {
		if (!BakedQuadWriter.write(this, pose, quad, instance)) {
			VertexConsumer.super.putBakedQuad(pose, quad, instance);
		}
	}
}
