pub mod journal;
pub mod lockbuilder;
pub mod base64;
pub mod ai;

use crate::journal::*;

use parry2d::math::Vec2;

use std::rc::Rc;
use std::cell::RefCell;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);
}

pub trait FromJSObject {
    fn from_js_object(obj: &js_sys::Object) -> Option<Self>
    where
        Self: Sized;
}

#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageElementArgTypesJSLabel {
    TextContent = "TextContent",
    ColorPicker = "ColorPicker",
    NumberRange = "NumberRange",
}

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct TextContent {}

impl FromJSObject for TextContent {
    fn from_js_object(_obj: &js_sys::Object) -> Option<Self> {
        Some(TextContent {})
    }
}

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct NumberRange {
    pub min: f32,
    pub max: f32,
}

impl FromJSObject for NumberRange {
    fn from_js_object(obj: &js_sys::Object) -> Option<Self> {
        let min = js_sys::Reflect::get(obj, &JsValue::from_str("min"))
            .ok()?
            .as_f64()? as f32;
        let max = js_sys::Reflect::get(obj, &JsValue::from_str("max"))
            .ok()?
            .as_f64()? as f32;

        Some(NumberRange { min, max })
    }
}

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct PageElementArgTypesJS {
    pub label: PageElementArgTypesJSLabel,
    #[wasm_bindgen(getter_with_clone)]
    pub arg_options: js_sys::Object,
}

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct PageElementArgJS {
    #[wasm_bindgen(getter_with_clone)]
    pub name: String,
    #[wasm_bindgen(getter_with_clone)]
    pub label: String,
    #[wasm_bindgen(getter_with_clone)]
    pub arg_type: PageElementArgTypesJS,
}

impl Into<PageElementArgType> for PageElementArgTypesJS {
    fn into(self) -> PageElementArgType {
        match self.label {
            PageElementArgTypesJSLabel::TextContent => {
                PageElementArgType::TextContent
            }
            PageElementArgTypesJSLabel::ColorPicker => {
                PageElementArgType::ColorPicker
            }
            PageElementArgTypesJSLabel::NumberRange => {
                let number_range = NumberRange::from_js_object(&self.arg_options).unwrap();
                PageElementArgType::NumberRange {
                    min: number_range.min,
                    max: number_range.max,
                }
            }
            _ => {
                panic!("Unknown PageElementArgTypesJSLabel: {:?}", self.label);
            }
        }
    }
}

impl Into<PageElementArgTypesJS> for PageElementArgType {
    fn into(self) -> PageElementArgTypesJS {
        let obj = js_sys::Object::new();
        match self {
            PageElementArgType::TextContent {} => {
                PageElementArgTypesJS {
                    label: PageElementArgTypesJSLabel::TextContent,
                    arg_options: obj,
                }
            },
            PageElementArgType::ColorPicker => {
                PageElementArgTypesJS {
                    label: PageElementArgTypesJSLabel::ColorPicker,
                    arg_options: obj,
                }
            },
            PageElementArgType::NumberRange { min, max } => {
                js_sys::Reflect::set(&obj, &JsValue::from_str("min"), &JsValue::from_f64(min as f64)).expect("Failed to set min");
                js_sys::Reflect::set(&obj, &JsValue::from_str("max"), &JsValue::from_f64(max as f64)).expect("Failed to set max");
                PageElementArgTypesJS {
                    label: PageElementArgTypesJSLabel::NumberRange,
                    arg_options: obj,
                }
            },
        }
    }
}

impl Into<PageElementArg> for PageElementArgJS {
    fn into(self) -> PageElementArg {
        PageElementArg {
            name: self.name,
            label: self.label,
            arg_type: self.arg_type.into(),
        }
    }
}

impl Into<PageElementArgJS> for PageElementArg {
    fn into(self) -> PageElementArgJS {
        PageElementArgJS {
            name: self.name,
            label: self.label,
            arg_type: self.arg_type.into(),
        }
    }
}

impl Into<PageElementType> for PageElementTypeJS {
    fn into(self) -> PageElementType {
        PageElementType {
            name: self.name,
            args: self.args.into_iter().map(|arg| arg.into()).collect(),
        }
    }
}

impl Into<PageElementTypeJS> for PageElementType {
    fn into(self) -> PageElementTypeJS {
        PageElementTypeJS {
            name: self.name,
            args: self.args.into_iter().map(|arg| arg.into()).collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum PageElementArgType {
    TextContent,
    ColorPicker,
    NumberRange {
        min: f32,
        max: f32,
    },
}

#[derive(Debug, Clone)]
pub struct PageElementArg {
    pub name: String,
    pub label: String,
    pub arg_type: PageElementArgType
}

#[derive(Debug, Clone)]
pub struct PageElementType {
    pub name: String,
    pub args: Vec<PageElementArg>,
}

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct PageElementTypeJS {
    #[wasm_bindgen(getter_with_clone)]
    pub name: String,
    #[wasm_bindgen(getter_with_clone)]
    pub args: Vec<PageElementArgJS>,
}

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct PageElementJS {
    #[wasm_bindgen(getter_with_clone)]
    pub pe_type: PageElementTypeJS,
    pub geom: PageElementGeomJS,
    #[wasm_bindgen(getter_with_clone)]
    pub object: js_sys::Object,
}

#[derive(Debug, Clone)]
pub struct PageElement {
    pub pe_type: PageElementType,
    pub geom: PageElementGeom,
    pub object: js_sys::Object,
}

impl From<PageElementJS> for PageElement {
    fn from(pe_js: PageElementJS) -> Self {
        PageElement {
            pe_type: pe_js.pe_type.into(),
            geom: pe_js.geom.into(),
            object: pe_js.object,
        }
    }
}

impl From<PageElement> for PageElementJS {
    fn from(pe: PageElement) -> Self {
        PageElementJS {
            pe_type: pe.pe_type.into(),
            geom: pe.geom.into(),
            object: pe.object,
        }
    }
}

#[wasm_bindgen]
pub fn fetch_page_element_types() -> Vec<PageElementTypeJS> {
    vec![
        PageElementType {
            name: "Text Box".into(),
            args: vec![
                PageElementArg {
                    name: "Content".into(),
                    label: "content".into(),
                    arg_type: PageElementArgType::TextContent,
                },
                PageElementArg {
                    name: "Font Size".into(),
                    label: "font_size".into(),
                    arg_type: PageElementArgType::NumberRange {
                        min: 8.0,
                        max: 72.0,
                    },
                },
                PageElementArg {
                    name: "Font Color".into(),
                    label: "font_color".into(),
                    arg_type: PageElementArgType::ColorPicker,
                },
            ],
        }.into(),
        PageElementType {
            name: "Symbol".into(),
            args: vec![
                PageElementArg {
                    name: "Path".into(),
                    label: "path".into(),
                    arg_type: PageElementArgType::TextContent,
                },
                PageElementArg {
                    name: "Symbol Size".into(),
                    label: "symbol_size".into(),
                    arg_type: PageElementArgType::NumberRange {
                        min: 8.0,
                        max: 72.0,
                    },
                },
                PageElementArg {
                    name: "Symbol Color".into(),
                    label: "symbol_color".into(),
                    arg_type: PageElementArgType::ColorPicker,
                },
            ],
        }.into(),
        // Add more types as needed
    ]
}

#[wasm_bindgen]
pub fn create_page_element(
    page_element_type: &PageElementTypeJS,
    page_element_geom: PageElementGeomJS,
    object: js_sys::Object,
) -> PageElementJS {
    // for every arg in pe_type, check if the object has that arg
    for arg in &page_element_type.args {
        if !js_sys::Reflect::has(&object, &JsValue::from_str(&arg.label)).unwrap_or(false) {
            log(&format!("Missing argument: {}", arg.name));
        }
    }
    PageElementJS {
        pe_type: page_element_type.clone(),
        geom: page_element_geom,
        object,
    }
}

#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub struct PageElementGeomJS {
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
    pub rotation: f32,
    pub scale: f32,
}

#[wasm_bindgen]
impl PageElementGeomJS {
    #[wasm_bindgen(constructor)]
    pub fn new(width: f32, height: f32, x: f32, y: f32, rotation: f32, scale: f32) -> PageElementGeomJS {
        PageElementGeomJS {
            width,
            height,
            x,
            y,
            rotation,
            scale,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PageElementGeom {
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
    pub rotation: f32,
    pub scale: f32,
}

impl PageElementGeom {
    pub fn new(width: f32, height: f32, x: f32, y: f32, rotation: f32, scale: f32) -> Self {
        PageElementGeom {
            width,
            height,
            x,
            y,
            rotation,
            scale,
        }
    }
}

impl From<PageElementGeomJS> for PageElementGeom {
    fn from(geom_js: PageElementGeomJS) -> Self {
        PageElementGeom {
            width: geom_js.width,
            height: geom_js.height,
            x: geom_js.x,
            y: geom_js.y,
            rotation: geom_js.rotation,
            scale: geom_js.scale,
        }
    }
}

impl From<PageElementGeom> for PageElementGeomJS {
    fn from(geom: PageElementGeom) -> Self {
        PageElementGeomJS {
            width: geom.width,
            height: geom.height,
            x: geom.x,
            y: geom.y,
            rotation: geom.rotation,
            scale: geom.scale,
        }
    }
}

#[wasm_bindgen]
pub fn render_page_elements(elems: Vec<PageElementJS>) -> Vec<u8> {
    log(&format!("Rendering {} page elements", elems.len()));
    for elem in &elems {
        log(&format!("Element: {:?}", elem));
    }
    vec![]
}
