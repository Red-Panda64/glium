/*!
Contains everything related to vertex buffers.

The main struct is the `VertexBuffer`, which represents a buffer in the video memory,
containing a list of vertices.

In order to create a vertex buffer, you must first create a struct that represents each vertex,
and implement the `glium::vertex_buffer::Vertex` trait on it. The `#[vertex_format]` attribute
located in `glium_macros` helps you with that.

```
# #![feature(plugin)]
# #[plugin]
# extern crate glium_macros;
# extern crate glium;
# extern crate glutin;
# fn main() {
#[vertex_format]
#[derive(Copy)]
struct Vertex {
    position: [f32; 3],
    texcoords: [f32; 2],
}
# }
```

Next, build a `Vec` of the vertices that you want to upload, and pass it to
`VertexBuffer::new`.

```no_run
# let display: glium::Display = unsafe { ::std::mem::uninitialized() };
# #[derive(Copy)]
# struct Vertex {
#     position: [f32; 3],
#     texcoords: [f32; 2],
# }
# impl glium::vertex_buffer::Vertex for Vertex {
#     fn build_bindings(_: Option<Vertex>) -> glium::vertex_buffer::VertexFormat {
#         unimplemented!() }
# }
let data = vec![
    Vertex {
        position: [0.0, 0.0, 0.4],
        texcoords: [0.0, 1.0]
    },
    Vertex {
        position: [12.0, 4.5, -1.8],
        texcoords: [1.0, 0.5]
    },
    Vertex {
        position: [-7.124, 0.1, 0.0],
        texcoords: [0.0, 0.4]
    },
];

let vertex_buffer = glium::vertex_buffer::VertexBuffer::new(&display, data);
```

*/
use buffer::{self, Buffer};
use sync::LinearSyncFence;
use std::ops::{Deref, DerefMut};
use std::sync::mpsc::Sender;
use gl;
use context;
use GlObject;

/// Describes the source to use for the vertices when drawing.
#[derive(Clone)]
pub enum VerticesSource<'a> {
    /// A buffer uploaded in the video memory.
    ///
    /// If the second parameter is `Some`, then a fence *must* be sent with this sender for
    /// when the buffer stops being used.
    VertexBuffer(&'a VertexBufferAny, Option<Sender<LinearSyncFence>>),
}

/// Objects that can be used as vertex sources.
pub trait IntoVerticesSource<'a> {
    /// Builds the `VerticesSource`.
    fn into_vertices_source(self) -> VerticesSource<'a>;
}

impl<'a> IntoVerticesSource<'a> for VerticesSource<'a> {
    fn into_vertices_source(self) -> VerticesSource<'a> {
        self
    }
}

/// A list of vertices loaded in the graphics card's memory.
#[derive(Show)]
pub struct VertexBuffer<T> {
    buffer: VertexBufferAny,
}

impl<T: Vertex + 'static + Send> VertexBuffer<T> {
    /// Builds a new vertex buffer.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # #![feature(plugin)]
    /// # #[plugin]
    /// # extern crate glium_macros;
    /// # extern crate glium;
    /// # extern crate glutin;
    /// # fn main() {
    /// #[vertex_format]
    /// #[derive(Copy)]
    /// struct Vertex {
    ///     position: [f32; 3],
    ///     texcoords: [f32; 2],
    /// }
    ///
    /// # let display: glium::Display = unsafe { ::std::mem::uninitialized() };
    /// let vertex_buffer = glium::VertexBuffer::new(&display, vec![
    ///     Vertex { position: [0.0,  0.0, 0.0], texcoords: [0.0, 1.0] },
    ///     Vertex { position: [5.0, -3.0, 2.0], texcoords: [1.0, 0.0] },
    /// ]);
    /// # }
    /// ```
    ///
    pub fn new(display: &super::Display, data: Vec<T>) -> VertexBuffer<T> {
        let bindings = Vertex::build_bindings(None::<T>);

        let buffer = Buffer::new::<buffer::ArrayBuffer, T>(display, data, false);
        let elements_size = buffer.get_elements_size();

        VertexBuffer {
            buffer: VertexBufferAny {
                buffer: buffer,
                bindings: bindings,
                elements_size: elements_size,
            }
        }
    }

    /// Builds a new vertex buffer.
    ///
    /// This function will create a buffer that has better performance when it is modified frequently.
    pub fn new_dynamic(display: &super::Display, data: Vec<T>) -> VertexBuffer<T> {
        let bindings = Vertex::build_bindings(None::<T>);

        let buffer = Buffer::new::<buffer::ArrayBuffer, T>(display, data, false);
        let elements_size = buffer.get_elements_size();

        VertexBuffer {
            buffer: VertexBufferAny {
                buffer: buffer,
                bindings: bindings,
                elements_size: elements_size,
            }
        }
    }

    /// Builds a new vertex buffer with persistent mapping.
    ///
    /// ## Features
    ///
    /// Only available if the `gl_persistent_mapping` feature is enabled.
    #[cfg(feature = "gl_persistent_mapping")]
    pub fn new_persistent(display: &super::Display, data: Vec<T>) -> VertexBuffer<T> {
        VertexBuffer::new_persistent_if_supported(display, data).unwrap()
    }

    /// Builds a new vertex buffer with persistent mapping, or `None` if this is not supported.
    pub fn new_persistent_if_supported(display: &super::Display, data: Vec<T>)
                                       -> Option<VertexBuffer<T>>
    {
        if display.context.context.get_version() < &context::GlVersion(4, 4) &&
           !display.context.context.get_extensions().gl_arb_buffer_storage
        {
            return None;
        }

        let bindings = Vertex::build_bindings(None::<T>);

        let buffer = Buffer::new::<buffer::ArrayBuffer, T>(display, data, true);
        let elements_size = buffer.get_elements_size();

        Some(VertexBuffer {
            buffer: VertexBufferAny {
                buffer: buffer,
                bindings: bindings,
                elements_size: elements_size,
            }
        })
    }
}

impl<T: Send + Copy> VertexBuffer<T> {
    /// Builds a new vertex buffer from an indeterminate data type and bindings.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # #![feature(plugin)]
    /// # #[plugin]
    /// # extern crate glium_macros;
    /// # extern crate glium;
    /// # extern crate glutin;
    /// # fn main() {
    /// let bindings = vec![(
    ///         format!("position"), 0,
    ///         glium::vertex_buffer::AttributeType::F32F32,
    ///     ), (
    ///         format!("color"), 2 * ::std::mem::size_of::<f32>(),
    ///         glium::vertex_buffer::AttributeType::F32,
    ///     ),
    /// ];
    ///
    /// # let display: glium::Display = unsafe { ::std::mem::uninitialized() };
    /// let data = vec![
    ///     1.0, -0.3, 409.0,
    ///     -0.4, 2.8, 715.0f32
    /// ];
    ///
    /// let vertex_buffer = unsafe {
    ///     glium::VertexBuffer::new_raw(&display, data, bindings, 3 * ::std::mem::size_of::<f32>())
    /// };
    /// # }
    /// ```
    ///
    #[experimental]
    pub unsafe fn new_raw(display: &super::Display, data: Vec<T>,
                          bindings: VertexFormat, elements_size: usize) -> VertexBuffer<T>
    {
        VertexBuffer {
            buffer: VertexBufferAny {
                buffer: Buffer::new::<buffer::ArrayBuffer, T>(display, data, false),
                bindings: bindings,
                elements_size: elements_size,
            }
        }
    }

    /// Maps the buffer to allow write access to it.
    ///
    /// This function will block until the buffer stops being used by the backend.
    /// This operation is much faster if the buffer is persistent.
    pub fn map<'a>(&'a mut self) -> Mapping<'a, T> {
        let len = self.buffer.buffer.get_elements_count();
        let mapping = self.buffer.buffer.map::<buffer::ArrayBuffer, T>(0, len);
        Mapping(mapping)
    }

    /// Reads the content of the buffer.
    ///
    /// This function is usually better if are just doing one punctual read, while `map`
    /// is better if you want to have multiple small reads.
    ///
    /// # Features
    ///
    /// Only available if the `gl_read_buffer` feature is enabled.
    #[cfg(feature = "gl_read_buffer")]
    pub fn read(&self) -> Vec<T> {
        self.buffer.buffer.read::<buffer::ArrayBuffer, T>()
    }

    /// Reads the content of the buffer.
    ///
    /// This function is usually better if are just doing one punctual read, while `map`
    /// is better if you want to have multiple small reads.
    pub fn read_if_supported(&self) -> Option<Vec<T>> {
        self.buffer.buffer.read_if_supported::<buffer::ArrayBuffer, T>()
    }

    /// Reads the content of the buffer.
    ///
    /// This function is usually better if are just doing one punctual read, while `map`
    /// is better if you want to have multiple small reads.
    ///
    /// The offset and size are expressed in number of elements.
    ///
    /// ## Panic
    ///
    /// Panics if `offset` or `offset + size` are greated than the size of the buffer.
    ///
    /// # Features
    ///
    /// Only available if the `gl_read_buffer` feature is enabled.
    #[cfg(feature = "gl_read_buffer")]
    pub fn read_slice(&self, offset: usize, size: usize) -> Vec<T> {
        self.buffer.buffer.read_slice::<buffer::ArrayBuffer, T>(offset, size)
    }

    /// Reads the content of the buffer.
    ///
    /// This function is usually better if are just doing one punctual read, while `map`
    /// is better if you want to have multiple small reads.
    ///
    /// The offset and size are expressed in number of elements.
    ///
    /// ## Panic
    ///
    /// Panics if `offset` or `offset + size` are greated than the size of the buffer.
    pub fn read_slice_if_supported(&self, offset: usize, size: usize) -> Option<Vec<T>> {
        self.buffer.buffer.read_slice_if_supported::<buffer::ArrayBuffer, T>(offset, size)
    }

    /// Writes some vertices to the buffer.
    ///
    /// Replaces some vertices in the buffer with others.
    /// The `offset` represents a number of vertices, not a number of bytes.
    pub fn write(&mut self, offset: usize, data: Vec<T>) {
        self.buffer.buffer.upload::<buffer::ArrayBuffer, _>(offset, data)
    }
}

impl<T> VertexBuffer<T> {
    /// Returns true if the buffer is mapped in a permanent way in memory.
    pub fn is_persistent(&self) -> bool {
        self.buffer.buffer.is_persistent()
    }

    /// Returns the number of bytes between two consecutive elements in the buffer.
    pub fn get_elements_size(&self) -> usize {
        self.buffer.elements_size
    }

    /// Returns the associated `VertexFormat`.
    pub fn get_bindings(&self) -> &VertexFormat {
        &self.buffer.bindings
    }

    /// Discard the type information and turn the vertex buffer into a `VertexBufferAny`.
    pub fn into_vertex_buffer_any(self) -> VertexBufferAny {
        self.buffer
    }
}

impl<T> GlObject for VertexBuffer<T> {
    fn get_id(&self) -> gl::types::GLuint {
        self.buffer.get_id()
    }
}

impl<'a, T> IntoVerticesSource<'a> for &'a VertexBuffer<T> {
    fn into_vertices_source(self) -> VerticesSource<'a> {
        (&self.buffer).into_vertices_source()
    }
}

/// A list of vertices loaded in the graphics card's memory.
///
/// Contrary to `VertexBuffer`, this struct doesn't know about the type of data
/// inside the buffer. Therefore you can't map or read it.
///
/// This struct is provided for convenience, so that you can have a `Vec<VertexBufferAny>`,
/// or return a `VertexBufferAny` instead of a `VertexBuffer<MyPrivateVertexType>`.
#[derive(Show)]
pub struct VertexBufferAny {
    buffer: Buffer,
    bindings: VertexFormat,
    elements_size: usize,
}

impl VertexBufferAny {
    /// Returns the number of bytes between two consecutive elements in the buffer.
    pub fn get_elements_size(&self) -> usize {
        self.elements_size
    }

    /// Returns the associated `VertexFormat`.
    pub fn get_bindings(&self) -> &VertexFormat {
        &self.bindings
    }

    /// Turns the vertex buffer into a `VertexBuffer` without checking the type.
    pub unsafe fn into_vertex_buffer<T>(self) -> VertexBuffer<T> {
        VertexBuffer {
            buffer: self,
        }
    }
}

impl Drop for VertexBufferAny {
    fn drop(&mut self) {
        // removing VAOs which contain this vertex buffer
        let mut vaos = self.buffer.get_display().context.vertex_array_objects.lock().unwrap();
        let to_delete = vaos.keys().filter(|&&(v, _, _)| v == self.buffer.get_id())
            .map(|k| k.clone()).collect::<Vec<_>>();
        for k in to_delete.into_iter() {
            vaos.remove(&k);
        }
    }
}

impl GlObject for VertexBufferAny {
    fn get_id(&self) -> gl::types::GLuint {
        self.buffer.get_id()
    }
}

impl<'a> IntoVerticesSource<'a> for &'a VertexBufferAny {
    fn into_vertices_source(self) -> VerticesSource<'a> {
        let fence = if self.buffer.is_persistent() {
            Some(self.buffer.add_fence())
        } else {
            None
        };

        VerticesSource::VertexBuffer(self, fence)
    }
}

/// A mapping of a buffer.
pub struct Mapping<'a, T>(buffer::Mapping<'a, buffer::ArrayBuffer, T>);

impl<'a, T> Deref for Mapping<'a, T> {
    type Target = [T];
    fn deref<'b>(&'b self) -> &'b [T] {
        self.0.deref()
    }
}

impl<'a, T> DerefMut for Mapping<'a, T> {
    fn deref_mut<'b>(&'b mut self) -> &'b mut [T] {
        self.0.deref_mut()
    }
}

#[allow(missing_docs)]
#[derive(Copy, Clone, Show, PartialEq, Eq)]
pub enum AttributeType {
    I8,
    I8I8,
    I8I8I8,
    I8I8I8I8,
    U8,
    U8U8,
    U8U8U8,
    U8U8U8U8,
    I16,
    I16I16,
    I16I16I16,
    I16I16I16I16,
    U16,
    U16U16,
    U16U16U16,
    U16U16U16U16,
    I32,
    I32I32,
    I32I32I32,
    I32I32I32I32,
    U32,
    U32U32,
    U32U32U32,
    U32U32U32U32,
    F32,
    F32F32,
    F32F32F32,
    F32F32F32F32,
}

/// Describes the layout of each vertex in a vertex buffer.
///
/// The first element is the name of the binding, the second element is the offset
/// from the start of each vertex to this element, and the third element is the type.
pub type VertexFormat = Vec<(String, usize, AttributeType)>;

/// Trait for structures that represent a vertex.
///
/// Instead of implementing this trait yourself, it is recommended to use the `#[vertex_format]`
/// attribute from `glium_macros` instead.
// TODO: this should be `unsafe`, but that would break the syntax extension
pub trait Vertex: Copy {
    /// Builds the `VertexFormat` representing the layout of this element.
    fn build_bindings(Option<Self>) -> VertexFormat;
}

/// Trait for types that can be used as vertex attributes.
pub unsafe trait Attribute {
    /// Get the type of data.
    fn get_type(_: Option<Self>) -> AttributeType;
}

unsafe impl Attribute for i8 {
    fn get_type(_: Option<i8>) -> AttributeType {
        AttributeType::I8
    }
}

unsafe impl Attribute for (i8, i8) {
    fn get_type(_: Option<(i8, i8)>) -> AttributeType {
        AttributeType::I8I8
    }
}

unsafe impl Attribute for [i8; 2] {
    fn get_type(_: Option<[i8; 2]>) -> AttributeType {
        AttributeType::I8I8
    }
}

unsafe impl Attribute for (i8, i8, i8) {
    fn get_type(_: Option<(i8, i8, i8)>) -> AttributeType {
        AttributeType::I8I8I8
    }
}

unsafe impl Attribute for [i8; 3] {
    fn get_type(_: Option<[i8; 3]>) -> AttributeType {
        AttributeType::I8I8I8
    }
}

unsafe impl Attribute for (i8, i8, i8, i8) {
    fn get_type(_: Option<(i8, i8, i8, i8)>) -> AttributeType {
        AttributeType::I8I8I8I8
    }
}

unsafe impl Attribute for [i8; 4] {
    fn get_type(_: Option<[i8; 4]>) -> AttributeType {
        AttributeType::I8I8I8I8
    }
}

unsafe impl Attribute for u8 {
    fn get_type(_: Option<u8>) -> AttributeType {
        AttributeType::U8
    }
}

unsafe impl Attribute for (u8, u8) {
    fn get_type(_: Option<(u8, u8)>) -> AttributeType {
        AttributeType::U8U8
    }
}

unsafe impl Attribute for [u8; 2] {
    fn get_type(_: Option<[u8; 2]>) -> AttributeType {
        AttributeType::U8U8
    }
}

unsafe impl Attribute for (u8, u8, u8) {
    fn get_type(_: Option<(u8, u8, u8)>) -> AttributeType {
        AttributeType::U8U8U8
    }
}

unsafe impl Attribute for [u8; 3] {
    fn get_type(_: Option<[u8; 3]>) -> AttributeType {
        AttributeType::U8U8U8
    }
}

unsafe impl Attribute for (u8, u8, u8, u8) {
    fn get_type(_: Option<(u8, u8, u8, u8)>) -> AttributeType {
        AttributeType::U8U8U8U8
    }
}

unsafe impl Attribute for [u8; 4] {
    fn get_type(_: Option<[u8; 4]>) -> AttributeType {
        AttributeType::U8U8U8U8
    }
}

unsafe impl Attribute for i16 {
    fn get_type(_: Option<i16>) -> AttributeType {
        AttributeType::I16
    }
}

unsafe impl Attribute for (i16, i16) {
    fn get_type(_: Option<(i16, i16)>) -> AttributeType {
        AttributeType::I16I16
    }
}

unsafe impl Attribute for [i16; 2] {
    fn get_type(_: Option<[i16; 2]>) -> AttributeType {
        AttributeType::I16I16
    }
}

unsafe impl Attribute for (i16, i16, i16) {
    fn get_type(_: Option<(i16, i16, i16)>) -> AttributeType {
        AttributeType::I16I16I16
    }
}

unsafe impl Attribute for [i16; 3] {
    fn get_type(_: Option<[i16; 3]>) -> AttributeType {
        AttributeType::I16I16I16
    }
}

unsafe impl Attribute for (i16, i16, i16, i16) {
    fn get_type(_: Option<(i16, i16, i16, i16)>) -> AttributeType {
        AttributeType::I16I16I16I16
    }
}

unsafe impl Attribute for [i16; 4] {
    fn get_type(_: Option<[i16; 4]>) -> AttributeType {
        AttributeType::I16I16I16I16
    }
}

unsafe impl Attribute for u16 {
    fn get_type(_: Option<u16>) -> AttributeType {
        AttributeType::U16
    }
}

unsafe impl Attribute for (u16, u16) {
    fn get_type(_: Option<(u16, u16)>) -> AttributeType {
        AttributeType::U16U16
    }
}

unsafe impl Attribute for [u16; 2] {
    fn get_type(_: Option<[u16; 2]>) -> AttributeType {
        AttributeType::U16U16
    }
}

unsafe impl Attribute for (u16, u16, u16) {
    fn get_type(_: Option<(u16, u16, u16)>) -> AttributeType {
        AttributeType::U16U16U16
    }
}

unsafe impl Attribute for [u16; 3] {
    fn get_type(_: Option<[u16; 3]>) -> AttributeType {
        AttributeType::U16U16U16
    }
}

unsafe impl Attribute for (u16, u16, u16, u16) {
    fn get_type(_: Option<(u16, u16, u16, u16)>) -> AttributeType {
        AttributeType::U16U16U16U16
    }
}

unsafe impl Attribute for [u16; 4] {
    fn get_type(_: Option<[u16; 4]>) -> AttributeType {
        AttributeType::U16U16U16U16
    }
}

unsafe impl Attribute for i32 {
    fn get_type(_: Option<i32>) -> AttributeType {
        AttributeType::I32
    }
}

unsafe impl Attribute for (i32, i32) {
    fn get_type(_: Option<(i32, i32)>) -> AttributeType {
        AttributeType::I32I32
    }
}

unsafe impl Attribute for [i32; 2] {
    fn get_type(_: Option<[i32; 2]>) -> AttributeType {
        AttributeType::I32I32
    }
}

unsafe impl Attribute for (i32, i32, i32) {
    fn get_type(_: Option<(i32, i32, i32)>) -> AttributeType {
        AttributeType::I32I32I32
    }
}

unsafe impl Attribute for [i32; 3] {
    fn get_type(_: Option<[i32; 3]>) -> AttributeType {
        AttributeType::I32I32I32
    }
}

unsafe impl Attribute for (i32, i32, i32, i32) {
    fn get_type(_: Option<(i32, i32, i32, i32)>) -> AttributeType {
        AttributeType::I32I32I32I32
    }
}

unsafe impl Attribute for [i32; 4] {
    fn get_type(_: Option<[i32; 4]>) -> AttributeType {
        AttributeType::I32I32I32I32
    }
}

unsafe impl Attribute for u32 {
    fn get_type(_: Option<u32>) -> AttributeType {
        AttributeType::U32
    }
}

unsafe impl Attribute for (u32, u32) {
    fn get_type(_: Option<(u32, u32)>) -> AttributeType {
        AttributeType::U32U32
    }
}

unsafe impl Attribute for [u32; 2] {
    fn get_type(_: Option<[u32; 2]>) -> AttributeType {
        AttributeType::U32U32
    }
}

unsafe impl Attribute for (u32, u32, u32) {
    fn get_type(_: Option<(u32, u32, u32)>) -> AttributeType {
        AttributeType::U32U32U32
    }
}

unsafe impl Attribute for [u32; 3] {
    fn get_type(_: Option<[u32; 3]>) -> AttributeType {
        AttributeType::U32U32U32
    }
}

unsafe impl Attribute for (u32, u32, u32, u32) {
    fn get_type(_: Option<(u32, u32, u32, u32)>) -> AttributeType {
        AttributeType::U32U32U32U32
    }
}

unsafe impl Attribute for [u32; 4] {
    fn get_type(_: Option<[u32; 4]>) -> AttributeType {
        AttributeType::U32U32U32U32
    }
}

unsafe impl Attribute for f32 {
    fn get_type(_: Option<f32>) -> AttributeType {
        AttributeType::F32
    }
}

unsafe impl Attribute for (f32, f32) {
    fn get_type(_: Option<(f32, f32)>) -> AttributeType {
        AttributeType::F32F32
    }
}

unsafe impl Attribute for [f32; 2] {
    fn get_type(_: Option<[f32; 2]>) -> AttributeType {
        AttributeType::F32F32
    }
}

unsafe impl Attribute for (f32, f32, f32) {
    fn get_type(_: Option<(f32, f32, f32)>) -> AttributeType {
        AttributeType::F32F32F32
    }
}

unsafe impl Attribute for [f32; 3] {
    fn get_type(_: Option<[f32; 3]>) -> AttributeType {
        AttributeType::F32F32F32
    }
}

unsafe impl Attribute for (f32, f32, f32, f32) {
    fn get_type(_: Option<(f32, f32, f32, f32)>) -> AttributeType {
        AttributeType::F32F32F32F32
    }
}

unsafe impl Attribute for [f32; 4] {
    fn get_type(_: Option<[f32; 4]>) -> AttributeType {
        AttributeType::F32F32F32F32
    }
}
