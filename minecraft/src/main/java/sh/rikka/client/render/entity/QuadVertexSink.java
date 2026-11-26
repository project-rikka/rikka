package sh.rikka.client.render.entity;

import com.mojang.renderpearl.api.vertex.VertexFormat;

public interface QuadVertexSink {
	long NO_RESERVATION = 0L;

	boolean rikka$takesQuads(VertexFormat format);

	long rikka$reserveVertices(int count);
}
