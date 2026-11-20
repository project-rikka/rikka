#include <ft2build.h>
#include FT_FREETYPE_H
#include FT_MULTIPLE_MASTERS_H
#include FT_OUTLINE_H

#include <stdint.h>
#include <stdlib.h>

typedef struct ft_bench {
	FT_Library library;
	FT_Face face;
} ft_bench;

typedef struct ft_bench_bitmap {
	uint32_t width;
	uint32_t rows;
	int32_t pitch;
	int32_t left;
	int32_t top;
	const uint8_t* buffer;
} ft_bench_bitmap;

ft_bench* ft_bench_open( const uint8_t* data, size_t length, long index )
{
	ft_bench* bench = calloc( 1, sizeof( ft_bench ) );
	if ( bench == NULL )
	{
		return NULL;
	}
	if ( FT_Init_FreeType( &bench->library ) != 0 )
	{
		free( bench );
		return NULL;
	}
	if ( FT_New_Memory_Face( bench->library, data, (FT_Long)length, index, &bench->face ) != 0 )
	{
		FT_Done_FreeType( bench->library );
		free( bench );
		return NULL;
	}
	return bench;
}

void ft_bench_close( ft_bench* bench )
{
	FT_Done_Face( bench->face );
	FT_Done_FreeType( bench->library );
	free( bench );
}

int ft_bench_set_axis( ft_bench* bench, uint32_t tag, float value )
{
	FT_MM_Var* master = NULL;
	if ( FT_Get_MM_Var( bench->face, &master ) != 0 )
	{
		return 0;
	}
	FT_Fixed* coords = malloc( master->num_axis * sizeof( FT_Fixed ) );
	int found = 0;
	if ( coords != NULL && FT_Get_Var_Design_Coordinates( bench->face, master->num_axis, coords ) == 0 )
	{
		for ( FT_UInt i = 0; i < master->num_axis; ++i )
		{
			if ( master->axis[i].tag == tag )
			{
				coords[i] = (FT_Fixed)( value * 65536.0f );
				found = 1;
			}
		}
		if ( found && FT_Set_Var_Design_Coordinates( bench->face, master->num_axis, coords ) != 0 )
		{
			found = 0;
		}
	}
	free( coords );
	FT_Done_MM_Var( bench->library, master );
	return found;
}

int ft_bench_set_size( ft_bench* bench, float ppem )
{
	return FT_Set_Char_Size( bench->face, 0, (FT_F26Dot6)( ppem * 64.0f + 0.5f ), 72, 72 ) == 0;
}

int ft_bench_render( ft_bench* bench, uint32_t glyph, float offset_x, ft_bench_bitmap* out )
{
	if ( FT_Load_Glyph( bench->face, glyph, FT_LOAD_NO_HINTING | FT_LOAD_NO_BITMAP ) != 0 )
	{
		return 0;
	}
	FT_GlyphSlot slot = bench->face->glyph;
	if ( slot->format == FT_GLYPH_FORMAT_OUTLINE && offset_x != 0.0f )
	{
		FT_Outline_Translate( &slot->outline, (FT_Pos)( offset_x * 64.0f + 0.5f ), 0 );
	}
	if ( FT_Render_Glyph( slot, FT_RENDER_MODE_NORMAL ) != 0 )
	{
		return 0;
	}
	out->width = slot->bitmap.width;
	out->rows = slot->bitmap.rows;
	out->pitch = slot->bitmap.pitch;
	out->left = slot->bitmap_left;
	out->top = slot->bitmap_top;
	out->buffer = slot->bitmap.buffer;
	return 1;
}

static int ft_bench_move( const FT_Vector* to, void* user )
{
	(void)to;
	++*(long*)user;
	return 0;
}

static int ft_bench_line( const FT_Vector* to, void* user )
{
	(void)to;
	++*(long*)user;
	return 0;
}

static int ft_bench_conic( const FT_Vector* control, const FT_Vector* to, void* user )
{
	(void)control;
	(void)to;
	++*(long*)user;
	return 0;
}

static int ft_bench_cubic( const FT_Vector* first, const FT_Vector* second, const FT_Vector* to, void* user )
{
	(void)first;
	(void)second;
	(void)to;
	++*(long*)user;
	return 0;
}

long ft_bench_outline( ft_bench* bench, uint32_t glyph )
{
	if ( FT_Load_Glyph( bench->face, glyph, FT_LOAD_NO_SCALE | FT_LOAD_NO_HINTING | FT_LOAD_NO_BITMAP ) != 0 )
	{
		return -1;
	}
	FT_GlyphSlot slot = bench->face->glyph;
	if ( slot->format != FT_GLYPH_FORMAT_OUTLINE )
	{
		return 0;
	}
	static const FT_Outline_Funcs funcs = {
		ft_bench_move, ft_bench_line, ft_bench_conic, ft_bench_cubic, 0, 0,
	};
	long segments = 0;
	if ( FT_Outline_Decompose( &slot->outline, &funcs, &segments ) != 0 )
	{
		return -1;
	}
	return segments;
}
