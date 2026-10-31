// The Roblox layer (hstudio_rbx): Roblox's API on Horizon, so Roblox UI
// scripts and general Luau scripts run as script mods. `game`, `workspace`,
// `script`, `Instance.new` and the Instance tree with its properties,
// events and methods; the services a client script uses (Players,
// RunService, UserInputService, TweenService, Debris, HttpService,
// TextService, GuiService, StarterGui, CollectionService,
// ContextActionService, Lighting...); RBXScriptSignals and connections; and
// tweens. Each part is put onto what the studio already does: ScreenGuis
// are laid out by hstudio_rbx_layout and drawn with the design system's
// shapes and fonts (hstudio_rbx_draw), Parts in the Workspace are drawn in
// the world, the LocalPlayer's character and the Camera follow the player,
// keys and clicks become UserInputService's input, and `Raycast` steps
// through the world's blocks.
//
// One DataModel is shared by every mod, as one game is by every script in
// Roblox; what a mod makes (instances, connections, tweens, waiting
// threads) goes when it stops. Signals are deferred, as Roblox's are by
// default: a handler runs in a thread of its own once the code that fired
// it is done.

use super::hstudio::{GameApi, Owner};
use super::hstudio_lua::{
    Host, Lua, Native, Res, Table, TableRef, Thread, Throw, Userdata, Value, bad_arg, check_num, check_text, opt_num,
};
use super::hstudio_rbx_types::{
    BrickColor, CFrame, Color3, ColorSequence, EasingDirection, EasingStyle, Font, FontEnum, NumberRange,
    NumberSequence, Rect, TweenInfo, UDim, UDim2, Vector2, Vector3, ease,
};
use super::hstudio_rbx_values::{
    self as values, Data, EnumItem, as_c3, as_cf, as_font, as_item, as_tween, as_udim, as_udim2, as_v2, as_v3, c3, cf,
    coerce_enum, enum_item, enum_value, get, udim, udim2, v2, v3, wrap,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Host function ids: the metamethods, then one per method name.
pub const FIRST_ID: u32 = 1000;
pub const LAST_ID: u32 = 1999;
const INST_INDEX: u32 = 1000;
const INST_NEWINDEX: u32 = 1001;
const INST_TOSTRING: u32 = 1002;
const SIGNAL_INDEX: u32 = 1003;
const CONN_INDEX: u32 = 1004;
const SIGNAL_TOSTRING: u32 = 1005;
const INSTANCE_NEW: u32 = 1006;
const INSTANCE_FROM_EXISTING: u32 = 1007;
/// The GUI's screen: draw, move, click, release, key, text, wheel, close,
/// escape.
const SCREEN_DRAW: u32 = 1010;
const SCREEN_CLOSE: u32 = 1017;
const SCREEN_ESCAPE: u32 = 1018;
const METHOD_BASE: u32 = 1100;
const SIGNAL_METHOD_BASE: u32 = 1700;
const CONN_METHOD_BASE: u32 = 1750;

/// Instances at most (a runaway loop of Instance.new stops there).
pub const MAX_INSTANCES: usize = 100_000;
/// Connections at most, over every instance.
pub const MAX_CONNECTIONS: usize = 50_000;
/// Rounds of deferred events one frame runs (Roblox's re-entrancy limit).
pub const DEFER_ROUNDS: usize = 10;

// ---------------------------------------------------------------- classes

/// A property's type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum K {
    Bool,
    Num,
    Int,
    Str,
    V2,
    V3,
    CF,
    C3,
    UD,
    UD2,
    Enum(&'static str),
    Obj,
    Font,
    Brick,
    NumSeq,
    ColorSeq,
    Range,
    Rect,
    Func,
}

/// A property's default, as Instance.new gives it.
#[derive(Clone, Copy, Debug)]
pub enum D {
    Nil,
    B(bool),
    N(f64),
    S(&'static str),
    V2(f64, f64),
    V3(f64, f64, f64),
    CF0,
    /// 0 to 255 each.
    Rgb(f64, f64, f64),
    UD(f64, f64),
    UD2(f64, f64, f64, f64),
    E(&'static str),
    Font(&'static str),
    Brick(i64),
    Seq(f64),
    ColorSeq,
    Range(f64, f64),
    Rect0,
}

pub struct P {
    pub name: &'static str,
    pub k: K,
    pub d: D,
    /// Scripts can read it but not set it.
    pub ro: bool,
}

const fn p(name: &'static str, k: K, d: D) -> P {
    P { name, k, d, ro: false }
}
const fn ro(name: &'static str, k: K, d: D) -> P {
    P { name, k, d, ro: true }
}

pub struct C {
    pub name: &'static str,
    pub base: Option<&'static str>,
    /// Instance.new makes it.
    pub creatable: bool,
    /// One of game's services.
    pub service: bool,
    /// Takes properties it doesn't declare (a class the studio only keeps).
    pub lenient: bool,
    pub props: &'static [P],
    /// More of its own, shared with other classes (the text properties).
    pub more: &'static [P],
    pub events: &'static [&'static str],
}

const fn class(name: &'static str, base: &'static str, props: &'static [P], events: &'static [&'static str]) -> C {
    C {
        name,
        base: Some(base),
        creatable: true,
        service: false,
        lenient: false,
        props,
        more: &[],
        events,
    }
}
const fn abstract_class(
    name: &'static str,
    base: &'static str,
    props: &'static [P],
    events: &'static [&'static str],
) -> C {
    C {
        creatable: false,
        ..class(name, base, props, events)
    }
}
const fn service(name: &'static str, props: &'static [P], events: &'static [&'static str]) -> C {
    C {
        creatable: false,
        service: true,
        ..class(name, "Instance", props, events)
    }
}
const fn kept(name: &'static str, base: &'static str) -> C {
    C {
        lenient: true,
        ..class(name, base, &[], &[])
    }
}
/// A service with nothing to do here, which scripts ask for anyway: it
/// takes any property, and a member it hasn't is nil.
const fn lenient_service(name: &'static str) -> C {
    C {
        lenient: true,
        ..service(name, &[], &[])
    }
}

use D::{B, N, Rgb, S};
use K::{Bool, Int, Num, Obj, Str};

const GUI_OBJECT: &[P] = &[
    p("Position", K::UD2, D::UD2(0.0, 0.0, 0.0, 0.0)),
    p("Size", K::UD2, D::UD2(0.0, 0.0, 0.0, 0.0)),
    p("AnchorPoint", K::V2, D::V2(0.0, 0.0)),
    p("Rotation", Num, N(0.0)),
    p("Visible", Bool, B(true)),
    p("ZIndex", Int, N(1.0)),
    p("LayoutOrder", Int, N(0.0)),
    p("BackgroundColor3", K::C3, Rgb(163.0, 162.0, 165.0)),
    p("BackgroundTransparency", Num, N(0.0)),
    p("BorderColor3", K::C3, Rgb(27.0, 42.0, 53.0)),
    p("BorderSizePixel", Int, N(1.0)),
    p("BorderMode", K::Enum("BorderMode"), D::E("Outline")),
    p("ClipsDescendants", Bool, B(false)),
    p("Active", Bool, B(false)),
    p("Interactable", Bool, B(true)),
    p("Selectable", Bool, B(false)),
    p("AutomaticSize", K::Enum("AutomaticSize"), D::E("None")),
    p("SizeConstraint", K::Enum("SizeConstraint"), D::E("RelativeXY")),
    p("Draggable", Bool, B(false)),
];

const TEXT: &[P] = &[
    p("TextColor3", K::C3, Rgb(27.0, 42.0, 53.0)),
    p("TextSize", Num, N(8.0)),
    p("TextScaled", Bool, B(false)),
    p("TextWrapped", Bool, B(false)),
    p("TextWrap", Bool, B(false)),
    p("TextTransparency", Num, N(0.0)),
    p("TextStrokeColor3", K::C3, Rgb(0.0, 0.0, 0.0)),
    p("TextStrokeTransparency", Num, N(1.0)),
    p("TextXAlignment", K::Enum("TextXAlignment"), D::E("Center")),
    p("TextYAlignment", K::Enum("TextYAlignment"), D::E("Center")),
    p("TextTruncate", K::Enum("TextTruncate"), D::E("None")),
    p("FontFace", K::Font, D::Font("Legacy")),
    p("RichText", Bool, B(false)),
    p("LineHeight", Num, N(1.0)),
    p("MaxVisibleGraphemes", Int, N(-1.0)),
    ro("TextBounds", K::V2, D::V2(0.0, 0.0)),
    ro("TextFits", Bool, B(false)),
];

const BUTTON: &[P] = &[
    p("Active", Bool, B(true)),
    p("AutoButtonColor", Bool, B(true)),
    p("Modal", Bool, B(false)),
    p("Selected", Bool, B(false)),
];
const BUTTON_EVENTS: &[&str] = &[
    "Activated",
    "MouseButton1Click",
    "MouseButton1Down",
    "MouseButton1Up",
    "MouseButton2Click",
    "MouseButton2Down",
    "MouseButton2Up",
];

const IMAGE: &[P] = &[
    p("Image", Str, S("")),
    p("ImageColor3", K::C3, Rgb(255.0, 255.0, 255.0)),
    p("ImageTransparency", Num, N(0.0)),
    p("ScaleType", K::Enum("ScaleType"), D::E("Stretch")),
    p("ImageRectOffset", K::V2, D::V2(0.0, 0.0)),
    p("ImageRectSize", K::V2, D::V2(0.0, 0.0)),
    p("SliceCenter", K::Rect, D::Rect0),
    p("SliceScale", Num, N(1.0)),
    p("ResampleMode", K::Enum("ResamplerMode"), D::E("Default")),
    p("TileSize", K::UD2, D::UD2(1.0, 0.0, 1.0, 0.0)),
];

/// An ImageButton's images while the pointer is over it and while it is pressed.
const IMAGE_BUTTON: &[P] = &[p("HoverImage", Str, S("")), p("PressedImage", Str, S(""))];

const PART: &[P] = &[
    p("CFrame", K::CF, D::CF0),
    p("Size", K::V3, D::V3(4.0, 1.0, 2.0)),
    p("Color", K::C3, Rgb(163.0, 162.0, 165.0)),
    p("Transparency", Num, N(0.0)),
    p("Reflectance", Num, N(0.0)),
    p("Material", K::Enum("Material"), D::E("Plastic")),
    p("Anchored", Bool, B(false)),
    p("CanCollide", Bool, B(true)),
    p("CanTouch", Bool, B(true)),
    p("CanQuery", Bool, B(true)),
    p("Locked", Bool, B(false)),
    p("Massless", Bool, B(false)),
    p("CastShadow", Bool, B(true)),
    p("AssemblyLinearVelocity", K::V3, D::V3(0.0, 0.0, 0.0)),
    p("AssemblyAngularVelocity", K::V3, D::V3(0.0, 0.0, 0.0)),
    p("Velocity", K::V3, D::V3(0.0, 0.0, 0.0)),
];

const VALUE_EVENTS: &[&str] = &["Changed"];

pub static CLASSES: &[C] = &[
    C {
        name: "Instance",
        base: None,
        creatable: false,
        service: false,
        lenient: false,
        props: &[p("Archivable", Bool, B(true))],
        more: &[],
        events: &[
            "Changed",
            "ChildAdded",
            "ChildRemoved",
            "DescendantAdded",
            "DescendantRemoving",
            "AncestryChanged",
            "Destroying",
            "AttributeChanged",
        ],
    },
    C {
        name: "DataModel",
        base: Some("Instance"),
        creatable: false,
        service: false,
        lenient: false,
        props: &[
            ro("PlaceId", Num, N(0.0)),
            ro("GameId", Num, N(0.0)),
            ro("JobId", Str, S("")),
            ro("CreatorId", Num, N(0.0)),
            ro("PlaceVersion", Num, N(0.0)),
        ],
        more: &[],
        events: &[],
    },
    service(
        "Workspace",
        &[
            p("Gravity", Num, N(196.2)),
            p("CurrentCamera", Obj, D::Nil),
            ro("Terrain", Obj, D::Nil),
            ro("DistributedGameTime", Num, N(0.0)),
            p("FallenPartsDestroyHeight", Num, N(-500.0)),
        ],
        &[],
    ),
    service(
        "Players",
        &[
            ro("LocalPlayer", Obj, D::Nil),
            ro("MaxPlayers", Int, N(1.0)),
            p("RespawnTime", Num, N(5.0)),
        ],
        &["PlayerAdded", "PlayerRemoving"],
    ),
    service(
        "RunService",
        &[],
        &[
            "Heartbeat",
            "RenderStepped",
            "Stepped",
            "PreRender",
            "PostSimulation",
            "PreSimulation",
            "PreAnimation",
        ],
    ),
    service(
        "UserInputService",
        &[
            p("MouseBehavior", K::Enum("MouseBehavior"), D::E("Default")),
            p("MouseIconEnabled", Bool, B(true)),
            p("MouseDeltaSensitivity", Num, N(1.0)),
            ro("MouseEnabled", Bool, B(true)),
            ro("KeyboardEnabled", Bool, B(true)),
            ro("TouchEnabled", Bool, B(false)),
            ro("GamepadEnabled", Bool, B(false)),
            ro("AccelerometerEnabled", Bool, B(false)),
            ro("VREnabled", Bool, B(false)),
        ],
        &[
            "InputBegan",
            "InputEnded",
            "InputChanged",
            "TextBoxFocused",
            "TextBoxFocusReleased",
            "WindowFocused",
            "WindowFocusReleased",
            "JumpRequest",
        ],
    ),
    service("TweenService", &[], &[]),
    service("Debris", &[p("MaxItems", Int, N(1000.0))], &[]),
    service("HttpService", &[ro("HttpEnabled", Bool, B(false))], &[]),
    service("TextService", &[], &[]),
    service(
        "GuiService",
        &[p("SelectedObject", Obj, D::Nil), ro("MenuIsOpen", Bool, B(false))],
        &["MenuOpened", "MenuClosed"],
    ),
    service("StarterGui", &[p("ShowDevelopmentGui", Bool, B(true))], &[]),
    service("CoreGui", &[], &[]),
    service("CollectionService", &[], &["TagAdded", "TagRemoved"]),
    service("ContextActionService", &[], &[]),
    service(
        "Lighting",
        &[
            p("ClockTime", Num, N(14.0)),
            p("TimeOfDay", Str, S("14:00:00")),
            p("Brightness", Num, N(2.0)),
            p("Ambient", K::C3, Rgb(70.0, 70.0, 70.0)),
            p("OutdoorAmbient", K::C3, Rgb(70.0, 70.0, 70.0)),
            p("FogEnd", Num, N(100000.0)),
            p("FogStart", Num, N(0.0)),
            p("FogColor", K::C3, Rgb(192.0, 192.0, 192.0)),
            p("GlobalShadows", Bool, B(true)),
            p("ExposureCompensation", Num, N(0.0)),
        ],
        &[],
    ),
    service("ReplicatedStorage", &[], &[]),
    service("ReplicatedFirst", &[], &[]),
    service("ServerStorage", &[], &[]),
    service("ServerScriptService", &[], &[]),
    service("StarterPack", &[], &[]),
    service(
        "StarterPlayer",
        &[
            p("CameraMaxZoomDistance", Num, N(128.0)),
            p("CameraMinZoomDistance", Num, N(0.5)),
        ],
        &[],
    ),
    service("Teams", &[], &[]),
    service("SoundService", &[p("RespectFilteringEnabled", Bool, B(true))], &[]),
    service("Chat", &[], &[]),
    service("TextChatService", &[], &[]),
    service("MarketplaceService", &[], &[]),
    service("LogService", &[], &["MessageOut"]),
    // Services scripts get at their top, though they can do nothing here.
    lenient_service("TeleportService"),
    lenient_service("VirtualUser"),
    lenient_service("VirtualInputManager"),
    lenient_service("Stats"),
    lenient_service("PathfindingService"),
    lenient_service("PhysicsService"),
    lenient_service("SocialService"),
    lenient_service("BadgeService"),
    lenient_service("GroupService"),
    lenient_service("LocalizationService"),
    lenient_service("PolicyService"),
    lenient_service("ProximityPromptService"),
    lenient_service("VRService"),
    lenient_service("HapticService"),
    lenient_service("GamepadService"),
    lenient_service("AssetService"),
    lenient_service("InsertService"),
    lenient_service("MaterialService"),
    abstract_class(
        "Player",
        "Instance",
        &[
            ro("UserId", Num, N(1.0)),
            p("DisplayName", Str, S("Player1")),
            p("Character", Obj, D::Nil),
            p("Team", Obj, D::Nil),
            ro("AccountAge", Int, N(0.0)),
            p("CameraMaxZoomDistance", Num, N(128.0)),
            p("CameraMinZoomDistance", Num, N(0.5)),
        ],
        &["Chatted", "CharacterAdded", "CharacterRemoving", "Idled"],
    ),
    abstract_class(
        "PlayerGui",
        "Instance",
        &[ro("CurrentScreenOrientation", Str, S("LandscapeLeft"))],
        &[],
    ),
    abstract_class("PlayerScripts", "Instance", &[], &[]),
    abstract_class("Backpack", "Instance", &[], &[]),
    abstract_class(
        "PlayerMouse",
        "Instance",
        &[
            ro("X", Num, N(0.0)),
            ro("Y", Num, N(0.0)),
            ro("Hit", K::CF, D::CF0),
            ro("Target", Obj, D::Nil),
            ro("ViewSizeX", Num, N(0.0)),
            ro("ViewSizeY", Num, N(0.0)),
            p("Icon", Str, S("")),
            p("TargetFilter", Obj, D::Nil),
        ],
        &[
            "Button1Down",
            "Button1Up",
            "Button2Down",
            "Button2Up",
            "Move",
            "WheelForward",
            "WheelBackward",
            "Idle",
        ],
    ),
    abstract_class(
        "InputObject",
        "Instance",
        &[
            ro("KeyCode", K::Enum("KeyCode"), D::E("Unknown")),
            ro("UserInputType", K::Enum("UserInputType"), D::E("None")),
            ro("UserInputState", K::Enum("UserInputState"), D::E("None")),
            ro("Position", K::V3, D::V3(0.0, 0.0, 0.0)),
            ro("Delta", K::V3, D::V3(0.0, 0.0, 0.0)),
        ],
        &[],
    ),
    class("Folder", "Instance", &[], &[]),
    class("Configuration", "Instance", &[], &[]),
    abstract_class("PVInstance", "Instance", &[], &[]),
    class(
        "Model",
        "PVInstance",
        &[p("PrimaryPart", Obj, D::Nil), p("WorldPivot", K::CF, D::CF0)],
        &[],
    ),
    abstract_class("BasePart", "PVInstance", PART, &["Touched", "TouchEnded"]),
    class(
        "Part",
        "BasePart",
        &[p("Shape", K::Enum("PartType"), D::E("Block"))],
        &[],
    ),
    class(
        "MeshPart",
        "BasePart",
        &[p("MeshId", Str, S("")), p("TextureID", Str, S(""))],
        &[],
    ),
    class("WedgePart", "BasePart", &[], &[]),
    class("CornerWedgePart", "BasePart", &[], &[]),
    class("TrussPart", "BasePart", &[], &[]),
    class(
        "SpawnLocation",
        "Part",
        &[p("Duration", Num, N(10.0)), p("Neutral", Bool, B(true))],
        &[],
    ),
    class("Seat", "Part", &[p("Disabled", Bool, B(false))], &[]),
    abstract_class(
        "Terrain",
        "BasePart",
        &[p("WaterColor", K::C3, Rgb(12.0, 84.0, 92.0))],
        &[],
    ),
    class(
        "Camera",
        "PVInstance",
        &[
            p("CFrame", K::CF, D::CF0),
            p("Focus", K::CF, D::CF0),
            p("FieldOfView", Num, N(70.0)),
            ro("ViewportSize", K::V2, D::V2(0.0, 0.0)),
            p("CameraType", K::Enum("CameraType"), D::E("Custom")),
            p("CameraSubject", Obj, D::Nil),
        ],
        &[],
    ),
    class(
        "Humanoid",
        "Instance",
        &[
            p("Health", Num, N(100.0)),
            p("MaxHealth", Num, N(100.0)),
            p("WalkSpeed", Num, N(16.0)),
            p("JumpPower", Num, N(50.0)),
            p("JumpHeight", Num, N(7.2)),
            p("UseJumpPower", Bool, B(true)),
            p("HipHeight", Num, N(2.0)),
            p("Sit", Bool, B(false)),
            p("Jump", Bool, B(false)),
            p("PlatformStand", Bool, B(false)),
            p("AutoRotate", Bool, B(true)),
            p("DisplayName", Str, S("")),
            p("RigType", K::Enum("HumanoidRigType"), D::E("R15")),
            ro("MoveDirection", K::V3, D::V3(0.0, 0.0, 0.0)),
            ro("FloorMaterial", K::Enum("Material"), D::E("Air")),
            p("RootPart", Obj, D::Nil),
        ],
        &[
            "Died",
            "HealthChanged",
            "Jumping",
            "Running",
            "StateChanged",
            "MoveToFinished",
            "Touched",
        ],
    ),
    abstract_class("ValueBase", "Instance", &[], VALUE_EVENTS),
    class("StringValue", "ValueBase", &[p("Value", Str, S(""))], &[]),
    class("NumberValue", "ValueBase", &[p("Value", Num, N(0.0))], &[]),
    class("IntValue", "ValueBase", &[p("Value", Int, N(0.0))], &[]),
    class("BoolValue", "ValueBase", &[p("Value", Bool, B(false))], &[]),
    class("ObjectValue", "ValueBase", &[p("Value", Obj, D::Nil)], &[]),
    class(
        "Vector3Value",
        "ValueBase",
        &[p("Value", K::V3, D::V3(0.0, 0.0, 0.0))],
        &[],
    ),
    class("CFrameValue", "ValueBase", &[p("Value", K::CF, D::CF0)], &[]),
    class(
        "Color3Value",
        "ValueBase",
        &[p("Value", K::C3, Rgb(0.0, 0.0, 0.0))],
        &[],
    ),
    class(
        "BrickColorValue",
        "ValueBase",
        &[p("Value", K::Brick, D::Brick(194))],
        &[],
    ),
    class("BindableEvent", "Instance", &[], &["Event"]),
    class("BindableFunction", "Instance", &[p("OnInvoke", K::Func, D::Nil)], &[]),
    class(
        "Sound",
        "Instance",
        &[
            p("SoundId", Str, S("")),
            p("Volume", Num, N(0.5)),
            p("PlaybackSpeed", Num, N(1.0)),
            p("Looped", Bool, B(false)),
            p("Playing", Bool, B(false)),
            p("TimePosition", Num, N(0.0)),
            ro("TimeLength", Num, N(0.0)),
            ro("IsPlaying", Bool, B(false)),
            p("PlayOnRemove", Bool, B(false)),
        ],
        &["Ended", "Played", "Paused", "Resumed", "Stopped"],
    ),
    abstract_class(
        "TweenBase",
        "Instance",
        &[ro("PlaybackState", K::Enum("PlaybackState"), D::E("Begin"))],
        &["Completed"],
    ),
    abstract_class("Tween", "TweenBase", &[ro("Instance", Obj, D::Nil)], &[]),
    abstract_class("LuaSourceContainer", "Instance", &[], &[]),
    abstract_class(
        "BaseScript",
        "LuaSourceContainer",
        &[p("Enabled", Bool, B(true)), p("Disabled", Bool, B(false))],
        &[],
    ),
    class("LocalScript", "BaseScript", &[], &[]),
    class("Script", "BaseScript", &[], &[]),
    class("ModuleScript", "LuaSourceContainer", &[], &[]),
    // The GUI classes.
    abstract_class(
        "GuiBase2d",
        "Instance",
        &[
            ro("AbsolutePosition", K::V2, D::V2(0.0, 0.0)),
            ro("AbsoluteSize", K::V2, D::V2(0.0, 0.0)),
            ro("AbsoluteRotation", Num, N(0.0)),
            p("AutoLocalize", Bool, B(true)),
        ],
        &[],
    ),
    abstract_class(
        "LayerCollector",
        "GuiBase2d",
        &[
            p("Enabled", Bool, B(true)),
            p("ResetOnSpawn", Bool, B(true)),
            p("ZIndexBehavior", K::Enum("ZIndexBehavior"), D::E("Sibling")),
        ],
        &[],
    ),
    class(
        "ScreenGui",
        "LayerCollector",
        &[
            p("DisplayOrder", Int, N(0.0)),
            p("IgnoreGuiInset", Bool, B(false)),
            p("ScreenInsets", K::Enum("ScreenInsets"), D::E("CoreUISafeInsets")),
            p("ClipToDeviceSafeArea", Bool, B(true)),
            p(
                "SafeAreaCompatibility",
                K::Enum("SafeAreaCompatibility"),
                D::E("FullscreenExtension"),
            ),
            p("OnTopOfCoreBlur", Bool, B(false)),
        ],
        &[],
    ),
    abstract_class(
        "GuiObject",
        "GuiBase2d",
        GUI_OBJECT,
        &[
            "InputBegan",
            "InputEnded",
            "InputChanged",
            "MouseEnter",
            "MouseLeave",
            "MouseMoved",
            "MouseWheelForward",
            "MouseWheelBackward",
        ],
    ),
    class("Frame", "GuiObject", &[], &[]),
    class(
        "CanvasGroup",
        "GuiObject",
        &[
            p("GroupTransparency", Num, N(0.0)),
            p("GroupColor3", K::C3, Rgb(255.0, 255.0, 255.0)),
        ],
        &[],
    ),
    C {
        more: TEXT,
        ..class("TextLabel", "GuiObject", &[p("Text", Str, S("Label"))], &[])
    },
    abstract_class("GuiButton", "GuiObject", BUTTON, BUTTON_EVENTS),
    C {
        more: TEXT,
        ..class("TextButton", "GuiButton", &[p("Text", Str, S("Button"))], &[])
    },
    C {
        more: TEXT,
        ..class(
            "TextBox",
            "GuiObject",
            &[
                p("Text", Str, S("TextBox")),
                p("Active", Bool, B(true)),
                p("PlaceholderText", Str, S("")),
                p("PlaceholderColor3", K::C3, Rgb(178.0, 178.0, 178.0)),
                p("ClearTextOnFocus", Bool, B(true)),
                p("MultiLine", Bool, B(false)),
                p("TextEditable", Bool, B(true)),
                p("CursorPosition", Int, N(1.0)),
                p("SelectionStart", Int, N(-1.0)),
                p("ShowNativeInput", Bool, B(true)),
            ],
            &["FocusLost", "Focused", "ReturnPressedFromOnScreenKeyboard"],
        )
    },
    class("ImageLabel", "GuiObject", IMAGE, &[]),
    C {
        more: IMAGE_BUTTON,
        ..class("ImageButton", "GuiButton", IMAGE, &[])
    },
    class(
        "ScrollingFrame",
        "GuiObject",
        &[
            p("CanvasSize", K::UD2, D::UD2(0.0, 0.0, 2.0, 0.0)),
            p("CanvasPosition", K::V2, D::V2(0.0, 0.0)),
            p("AutomaticCanvasSize", K::Enum("AutomaticSize"), D::E("None")),
            p("ScrollBarThickness", Int, N(12.0)),
            p("ScrollBarImageColor3", K::C3, Rgb(0.0, 0.0, 0.0)),
            p("ScrollBarImageTransparency", Num, N(0.0)),
            p("ScrollingEnabled", Bool, B(true)),
            p("ScrollingDirection", K::Enum("ScrollingDirection"), D::E("XY")),
            p("VerticalScrollBarInset", K::Enum("ScrollBarInset"), D::E("None")),
            p("HorizontalScrollBarInset", K::Enum("ScrollBarInset"), D::E("None")),
            p(
                "VerticalScrollBarPosition",
                K::Enum("VerticalScrollBarPosition"),
                D::E("Right"),
            ),
            p("ElasticBehavior", K::Enum("ElasticBehavior"), D::E("WhenScrollable")),
            // A ScrollingFrame clips what it holds unless told otherwise.
            p("ClipsDescendants", Bool, B(true)),
            p("TopImage", Str, S("")),
            p("MidImage", Str, S("")),
            p("BottomImage", Str, S("")),
            ro("AbsoluteCanvasSize", K::V2, D::V2(0.0, 0.0)),
            ro("AbsoluteWindowSize", K::V2, D::V2(0.0, 0.0)),
        ],
        &[],
    ),
    abstract_class("UIComponent", "Instance", &[], &[]),
    class(
        "UICorner",
        "UIComponent",
        &[p("CornerRadius", K::UD, D::UD(0.0, 8.0))],
        &[],
    ),
    class(
        "UIStroke",
        "UIComponent",
        &[
            p("Color", K::C3, Rgb(0.0, 0.0, 0.0)),
            p("Thickness", Num, N(1.0)),
            p("Transparency", Num, N(0.0)),
            p("Enabled", Bool, B(true)),
            p("ApplyStrokeMode", K::Enum("ApplyStrokeMode"), D::E("Contextual")),
            p("LineJoinMode", K::Enum("LineJoinMode"), D::E("Round")),
        ],
        &[],
    ),
    class(
        "UIGradient",
        "UIComponent",
        &[
            p("Color", K::ColorSeq, D::ColorSeq),
            p("Transparency", K::NumSeq, D::Seq(0.0)),
            p("Rotation", Num, N(0.0)),
            p("Offset", K::V2, D::V2(0.0, 0.0)),
            p("Enabled", Bool, B(true)),
        ],
        &[],
    ),
    class(
        "UIPadding",
        "UIComponent",
        &[
            p("PaddingTop", K::UD, D::UD(0.0, 0.0)),
            p("PaddingBottom", K::UD, D::UD(0.0, 0.0)),
            p("PaddingLeft", K::UD, D::UD(0.0, 0.0)),
            p("PaddingRight", K::UD, D::UD(0.0, 0.0)),
        ],
        &[],
    ),
    class("UIScale", "UIComponent", &[p("Scale", Num, N(1.0))], &[]),
    class(
        "UIAspectRatioConstraint",
        "UIComponent",
        &[
            p("AspectRatio", Num, N(1.0)),
            p("AspectType", K::Enum("AspectType"), D::E("FitWithinMaxSize")),
            p("DominantAxis", K::Enum("DominantAxis"), D::E("Width")),
        ],
        &[],
    ),
    class(
        "UISizeConstraint",
        "UIComponent",
        &[
            p("MinSize", K::V2, D::V2(0.0, 0.0)),
            p("MaxSize", K::V2, D::V2(f64::INFINITY, f64::INFINITY)),
        ],
        &[],
    ),
    class(
        "UITextSizeConstraint",
        "UIComponent",
        &[p("MinTextSize", Int, N(1.0)), p("MaxTextSize", Int, N(100.0))],
        &[],
    ),
    abstract_class(
        "UIGridStyleLayout",
        "UIComponent",
        &[
            p("HorizontalAlignment", K::Enum("HorizontalAlignment"), D::E("Left")),
            p("VerticalAlignment", K::Enum("VerticalAlignment"), D::E("Top")),
            p("SortOrder", K::Enum("SortOrder"), D::E("LayoutOrder")),
            ro("AbsoluteContentSize", K::V2, D::V2(0.0, 0.0)),
        ],
        &[],
    ),
    class(
        "UIListLayout",
        "UIGridStyleLayout",
        &[
            p("FillDirection", K::Enum("FillDirection"), D::E("Vertical")),
            p("Padding", K::UD, D::UD(0.0, 0.0)),
            p("Wraps", Bool, B(false)),
            p("HorizontalFlex", K::Enum("UIFlexAlignment"), D::E("None")),
            p("VerticalFlex", K::Enum("UIFlexAlignment"), D::E("None")),
            p("ItemLineAlignment", K::Enum("ItemLineAlignment"), D::E("Automatic")),
        ],
        &[],
    ),
    class(
        "UIGridLayout",
        "UIGridStyleLayout",
        &[
            p("FillDirection", K::Enum("FillDirection"), D::E("Horizontal")),
            p("CellSize", K::UD2, D::UD2(0.0, 100.0, 0.0, 100.0)),
            p("CellPadding", K::UD2, D::UD2(0.0, 5.0, 0.0, 5.0)),
            p("FillDirectionMaxCells", Int, N(0.0)),
            p("StartCorner", K::Enum("StartCorner"), D::E("TopLeft")),
            ro("AbsoluteCellSize", K::V2, D::V2(0.0, 0.0)),
            ro("AbsoluteCellCount", K::V2, D::V2(0.0, 0.0)),
        ],
        &[],
    ),
    // Drawn over a part, a model or an attachment in the Workspace, and
    // lenient as it was when only kept.
    C {
        lenient: true,
        ..class(
            "BillboardGui",
            "LayerCollector",
            &[
                p("Adornee", Obj, D::Nil),
                p("Size", K::UD2, D::UD2(0.0, 0.0, 0.0, 0.0)),
                p("SizeOffset", K::V2, D::V2(0.0, 0.0)),
                p("StudsOffset", K::V3, D::V3(0.0, 0.0, 0.0)),
                p("StudsOffsetWorldSpace", K::V3, D::V3(0.0, 0.0, 0.0)),
                p("ExtentsOffset", K::V3, D::V3(0.0, 0.0, 0.0)),
                p("ExtentsOffsetWorldSpace", K::V3, D::V3(0.0, 0.0, 0.0)),
                p("AlwaysOnTop", Bool, B(false)),
                p("MaxDistance", Num, N(f64::INFINITY)),
                p("DistanceLowerLimit", Num, N(0.0)),
                p("DistanceUpperLimit", Num, N(-1.0)),
                p("DistanceStep", Num, N(0.0)),
                p("LightInfluence", Num, N(1.0)),
                p("Brightness", Num, N(1.0)),
                p("ClipsDescendants", Bool, B(false)),
                p("Active", Bool, B(false)),
                p("PlayerToHideFrom", Obj, D::Nil),
            ],
            &[],
        )
    },
    // TextService:GetTextBoundsAsync's question.
    class(
        "GetTextBoundsParams",
        "Instance",
        &[
            p("Text", Str, S("")),
            p("Font", K::Font, D::Font("SourceSans")),
            p("Size", Num, N(8.0)),
            p("Width", Num, N(0.0)),
            p("RichText", Bool, B(false)),
        ],
        &[],
    ),
    // Kept with whatever properties scripts give them; nothing draws them.
    kept("SurfaceGui", "LayerCollector"),
    // Laid out and drawn as Frames (what they show, a 3D view or a video,
    // isn't), and lenient as they were when only kept.
    C {
        lenient: true,
        ..class(
            "ViewportFrame",
            "GuiObject",
            &[
                p("CurrentCamera", Obj, D::Nil),
                p("Ambient", K::C3, Rgb(200.0, 200.0, 200.0)),
                p("LightColor", K::C3, Rgb(140.0, 140.0, 140.0)),
                p("LightDirection", K::V3, D::V3(-1.0, -1.0, -1.0)),
                p("ImageColor3", K::C3, Rgb(255.0, 255.0, 255.0)),
                p("ImageTransparency", Num, N(0.0)),
            ],
            &[],
        )
    },
    C {
        lenient: true,
        ..class(
            "VideoFrame",
            "GuiObject",
            &[
                p("Video", Str, S("")),
                p("Playing", Bool, B(false)),
                p("Looped", Bool, B(false)),
                p("Volume", Num, N(1.0)),
                p("TimePosition", Num, N(0.0)),
                ro("TimeLength", Num, N(0.0)),
                ro("IsLoaded", Bool, B(false)),
                ro("Resolution", K::V2, D::V2(0.0, 0.0)),
            ],
            &["DidLoop", "Ended", "Loaded", "Paused", "Played"],
        )
    },
    // Real now, and lenient as they were when only kept.
    C {
        lenient: true,
        ..class(
            "UIFlexItem",
            "UIComponent",
            &[
                p("FlexMode", K::Enum("UIFlexMode"), D::E("None")),
                p("GrowRatio", Num, N(1.0)),
                p("ShrinkRatio", Num, N(1.0)),
                p("ItemLineAlignment", K::Enum("ItemLineAlignment"), D::E("Automatic")),
            ],
            &[],
        )
    },
    C {
        lenient: true,
        ..class(
            "UIPageLayout",
            "UIGridStyleLayout",
            &[
                p("FillDirection", K::Enum("FillDirection"), D::E("Horizontal")),
                p("HorizontalAlignment", K::Enum("HorizontalAlignment"), D::E("Center")),
                p("VerticalAlignment", K::Enum("VerticalAlignment"), D::E("Center")),
                p("Padding", K::UD, D::UD(0.0, 0.0)),
                p("Animated", Bool, B(true)),
                p("Circular", Bool, B(false)),
                p("EasingStyle", K::Enum("EasingStyle"), D::E("Back")),
                p("EasingDirection", K::Enum("EasingDirection"), D::E("Out")),
                p("TweenTime", Num, N(1.0)),
                p("ScrollWheelInputEnabled", Bool, B(true)),
                p("GamepadInputEnabled", Bool, B(true)),
                p("TouchInputEnabled", Bool, B(true)),
                ro("CurrentPage", Obj, D::Nil),
            ],
            &["PageEnter", "PageLeave", "Stopped"],
        )
    },
    C {
        lenient: true,
        ..class(
            "UITableLayout",
            "UIGridStyleLayout",
            &[
                p("FillDirection", K::Enum("FillDirection"), D::E("Vertical")),
                p("FillEmptySpaceColumns", Bool, B(false)),
                p("FillEmptySpaceRows", Bool, B(false)),
                p("MajorAxis", K::Enum("TableMajorAxis"), D::E("RowMajor")),
                p("Padding", K::UD2, D::UD2(0.0, 0.0, 0.0, 0.0)),
            ],
            &[],
        )
    },
    kept("Decal", "Instance"),
    kept("Texture", "Instance"),
    kept("SpecialMesh", "Instance"),
    kept("PointLight", "Instance"),
    kept("SpotLight", "Instance"),
    kept("SurfaceLight", "Instance"),
    kept("ParticleEmitter", "Instance"),
    kept("Fire", "Instance"),
    kept("Smoke", "Instance"),
    kept("Sparkles", "Instance"),
    kept("Beam", "Instance"),
    kept("Trail", "Instance"),
    kept("Attachment", "Instance"),
    kept("Weld", "Instance"),
    kept("WeldConstraint", "Instance"),
    kept("Motor6D", "Instance"),
    kept("Highlight", "Instance"),
    kept("SelectionBox", "Instance"),
    C {
        lenient: true,
        ..class(
            "ProximityPrompt",
            "Instance",
            &[
                p("ActionText", Str, S("Interact")),
                p("ObjectText", Str, S("")),
                p("HoldDuration", Num, N(0.0)),
                p("MaxActivationDistance", Num, N(10.0)),
                p("Enabled", Bool, B(true)),
                p("RequiresLineOfSight", Bool, B(true)),
                p("KeyboardKeyCode", K::Enum("KeyCode"), D::E("E")),
            ],
            &[
                "Triggered",
                "TriggerEnded",
                "PromptShown",
                "PromptHidden",
                "PromptButtonHoldBegan",
                "PromptButtonHoldEnded",
            ],
        )
    },
    C {
        lenient: true,
        ..class(
            "ClickDetector",
            "Instance",
            &[p("MaxActivationDistance", Num, N(32.0)), p("CursorIcon", Str, S(""))],
            &["MouseClick", "RightMouseClick", "MouseHoverEnter", "MouseHoverLeave"],
        )
    },
    abstract_class("WebSocket", "Instance", &[], &["OnMessage", "OnClose"]),
    kept("Animation", "Instance"),
    kept("Animator", "Instance"),
    kept("Shirt", "Instance"),
    kept("Pants", "Instance"),
    kept("Accessory", "Instance"),
    kept("Tool", "Instance"),
    kept("RemoteEvent", "Instance"),
    kept("RemoteFunction", "Instance"),
    kept("UnreliableRemoteEvent", "Instance"),
    kept("Team", "Instance"),
    kept("Sky", "Instance"),
    kept("Atmosphere", "Instance"),
    kept("BloomEffect", "Instance"),
    kept("BlurEffect", "Instance"),
    kept("ColorCorrectionEffect", "Instance"),
    kept("SunRaysEffect", "Instance"),
];

pub fn class_def(name: &str) -> Option<&'static C> {
    thread_local! {
        static BY_NAME: HashMap<&'static str, &'static C> = CLASSES.iter().map(|c| (c.name, c)).collect();
    }
    BY_NAME.with(|m| m.get(name).copied())
}

/// Whether class `c` is `name` or inherits it.
pub fn is_a(c: &'static C, name: &str) -> bool {
    let mut at = Some(c);
    while let Some(k) = at {
        if k.name == name {
            return true;
        }
        at = k.base.and_then(class_def);
    }
    false
}

fn find_prop(c: &'static C, name: &str) -> Option<&'static P> {
    let mut at = Some(c);
    while let Some(k) = at {
        if let Some(p) = k.props.iter().chain(k.more).find(|p| p.name == name) {
            return Some(p);
        }
        at = k.base.and_then(class_def);
    }
    None
}

fn has_event(c: &'static C, name: &str) -> bool {
    let mut at = Some(c);
    while let Some(k) = at {
        if k.events.contains(&name) {
            return true;
        }
        at = k.base.and_then(class_def);
    }
    false
}

/// Every property of a class, its own last.
fn all_props(c: &'static C) -> Vec<&'static P> {
    let mut chain = Vec::new();
    let mut at = Some(c);
    while let Some(k) = at {
        chain.push(k);
        at = k.base.and_then(class_def);
    }
    chain.iter().rev().flat_map(|k| k.props.iter().chain(k.more)).collect()
}

fn default_value(d: D) -> Value {
    match d {
        D::Nil => Value::Nil,
        D::B(b) => Value::Bool(b),
        D::N(n) => Value::Num(n),
        D::S(s) => Value::str(s),
        D::V2(x, y) => v2(Vector2::new(x, y)),
        D::V3(x, y, z) => v3(Vector3::new(x, y, z)),
        D::CF0 => cf(CFrame::IDENTITY),
        D::Rgb(r, g, b) => c3(Color3::from_rgb(r, g, b)),
        D::UD(s, o) => udim(UDim::new(s, o)),
        D::UD2(a, b, c, d) => udim2(UDim2::new(a, b, c, d)),
        D::E(_) => Value::Nil,
        D::Font(name) => wrap(Data::Font(
            FontEnum::from_name(name)
                .and_then(|e| Font::from_enum(e).ok())
                .unwrap_or_default(),
        )),
        D::Brick(n) => wrap(Data::Brick(BrickColor::new(n))),
        D::Seq(n) => wrap(Data::NumSeq(NumberSequence::constant(n))),
        D::ColorSeq => wrap(Data::ColorSeq(ColorSequence::constant(Color3::new(1.0, 1.0, 1.0)))),
        D::Range(a, b) => wrap(Data::Range(NumberRange::new(a, b).unwrap_or(NumberRange::single(a)))),
        D::Rect0 => wrap(Data::Rect(Rect::default())),
    }
}

fn prop_default(p: &P) -> Value {
    match (p.k, p.d) {
        (K::Enum(e), D::E(item)) => enum_value(enum_item(e, item)),
        _ => default_value(p.d),
    }
}

// ---------------------------------------------------------------- instances

pub type Id = u32;

/// What an Instance userdata holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstRef(pub Id);

/// What an RBXScriptSignal userdata holds: its instance and event.
#[derive(Clone, Debug)]
pub struct SignalRef {
    pub inst: Id,
    pub event: Rc<str>,
}

/// What an RBXScriptConnection userdata holds.
#[derive(Clone, Copy, Debug)]
pub struct ConnRef {
    pub inst: Id,
    pub id: u32,
}

pub struct Conn {
    pub id: u32,
    pub event: Rc<str>,
    pub owner: Owner,
    pub f: Value,
    pub once: bool,
    /// Disable() turned it off (getconnections).
    pub enabled: bool,
    /// The thread that connected it.
    pub thread: Option<Rc<Thread>>,
}

pub struct Inst {
    pub id: Id,
    pub class: &'static C,
    pub name: String,
    pub parent: Option<Id>,
    pub children: Vec<Id>,
    pub props: HashMap<&'static str, Value>,
    /// A lenient class's other properties.
    pub extra: HashMap<String, Value>,
    pub attributes: Vec<(String, Value)>,
    pub tags: Vec<String>,
    pub conns: Vec<Conn>,
    /// The one value scripts see for it.
    pub value: Value,
    /// Who made it: it goes when they stop (None: the studio's own).
    pub owner: Option<Owner>,
    pub destroyed: bool,
    /// The parent can't change (services, destroyed instances).
    pub locked: bool,
    /// Properties setscriptable turned off, and hidden ones it turned on.
    pub unscriptable: HashSet<String>,
    pub revealed: HashSet<String>,
    /// A GuiBase2d or a UI component: a change to it changes the GUI's layout.
    pub gui: bool,
    /// The GUI epoch its layout's outputs (AbsoluteSize, TextBounds...) were
    /// worked out at; 0 before they ever were.
    pub laid_out: u64,
    /// The GUI epoch a read beside it worked out where it is at (its
    /// AbsolutePosition, AbsoluteSize and AbsoluteRotation, not its text).
    pub placed_at: u64,
}

impl Inst {
    pub fn get(&self, name: &str) -> Value {
        self.props.get(name).cloned().unwrap_or_default()
    }
}

/// A thread waiting on the layer: for a signal, or a child.
pub struct Waiter {
    pub owner: Owner,
    pub thread: Rc<Thread>,
    pub what: Wait,
}

pub enum Wait {
    Signal(Id, Rc<str>),
    Child {
        parent: Id,
        name: String,
        until: Option<f64>,
        since: f64,
        warned: bool,
    },
}

/// A tween running on an instance's properties.
pub struct Tween {
    pub id: Id,
    pub target: Id,
    pub info: TweenInfo,
    pub goals: Vec<(&'static str, Value)>,
    pub starts: Vec<Value>,
    pub elapsed: f64,
    pub playing: bool,
    pub owner: Owner,
    /// GuiObject:TweenPosition's callback.
    pub callback: Option<Value>,
}

/// What the mouse and keys are doing, for the GUI and UserInputService.
#[derive(Default)]
pub struct Pointer {
    /// In dp, Roblox's GUI pixels here.
    pub x: f64,
    pub y: f64,
    /// While a ScreenGui has the pointer (its screen is open).
    pub free: bool,
    /// Escape closed it: stays closed until the GUI stops asking.
    pub dismissed: bool,
    /// Left Alt asked for it.
    pub asked: bool,
    pub hover: Vec<Id>,
    pub pressed: Option<(Id, bool)>,
    pub focus: Option<Id>,
    /// A Draggable object being moved: it, and the pointer's last place.
    pub drag: Option<(Id, f64, f64)>,
    /// The TextBox whose text the held button is selecting.
    pub selecting: Option<Id>,
    /// A ScrollingFrame's bar being dragged: the frame, whether it is the
    /// vertical bar, and where along the thumb the pointer holds it.
    pub bar: Option<(Id, bool, f64)>,
    pub held: HashSet<&'static str>,
    pub buttons: [bool; 3],
}

pub struct Rbx {
    pub insts: HashMap<Id, Inst>,
    next_id: Id,
    next_conn: u32,
    conn_count: usize,
    pub game: Id,
    pub workspace: Id,
    pub players: Id,
    pub local_player: Id,
    pub player_gui: Id,
    pub core_gui: Id,
    pub camera: Id,
    pub character: Id,
    pub root_part: Id,
    pub head: Id,
    pub humanoid: Id,
    pub mouse: Id,
    services: HashMap<&'static str, Id>,
    inst_meta: TableRef,
    signal_meta: TableRef,
    conn_meta: TableRef,
    natives: HashMap<u32, Value>,
    /// Handlers to run: (owner, function, arguments).
    pub deferred: Vec<(Owner, Value, Vec<Value>)>,
    /// Threads to resume with these values.
    pub ready: Vec<(Owner, Rc<Thread>, Vec<Value>)>,
    pub waiters: Vec<Waiter>,
    pub tweens: Vec<Tween>,
    pub debris: Vec<(Id, f64)>,
    /// ContextActionService's bindings: name, owner, function, inputs.
    pub actions: Vec<(String, Owner, Value, Vec<EnumItem>)>,
    /// RunService:BindToRenderStep's: name, priority, owner, function.
    pub render_steps: Vec<(String, f64, Owner, Value)>,
    /// StarterGui:SetCoreGuiEnabled's choices.
    pub core_gui_off: HashSet<&'static str>,
    pub pointer: Pointer,
    /// Warnings for the console (an infinite WaitForChild).
    pub warnings: Vec<String>,
    /// Chat lines and commands the layer sends (Player:Chat, teleports).
    pub sends: Vec<String>,
    /// Sounds to play: id, place, volume, pitch.
    pub sounds: Vec<(String, Option<[f64; 3]>, f32, f32)>,
    /// Agent commands to run (Humanoid:MoveTo).
    pub agent: Vec<String>,
    /// The studio's clock.
    pub now: f64,
    /// The layout of the last frame, for hits.
    pub layout: Option<super::hstudio_rbx_draw::Frame>,
    /// The player as the last frame saw it.
    pub last_health: Option<f32>,
    /// A script has used the layer: until then it costs the game nothing.
    pub active: bool,
    /// gethui's container: a Folder outside the DataModel whose ScreenGuis draw.
    pub hui: Id,
    /// HTTP requests on their way, with the threads that wait for them.
    pub http: Vec<HttpWait>,
    /// Open WebSockets.
    pub sockets: Vec<SocketEntry>,
    /// Drawing objects (Drawing.new).
    pub drawings: Vec<super::hstudio_sunc::Drawing>,
    pub next_drawing: u32,
    /// Methods written in Lua (the prelude's HttpGet and the like), by class.
    pub lua_methods: Vec<(&'static str, &'static str, Value)>,
    /// ModuleScripts `require` ran, and what they returned.
    pub loaded_modules: Vec<(Id, Value)>,
    /// The studio's workspace folder (getcustomasset's files), and the images
    /// the GUI and the Drawings draw, on the HUD or on the GUI's own screen.
    pub workspace_dir: std::path::PathBuf,
    pub screen_images: super::hstudio_sunc::ImageCache,
    /// Counts the changes that move the GUI's layout (a GUI instance's
    /// property, the Parent of what holds one, the screen's size, the
    /// TextBox focus), so that a read of AbsoluteSize, TextBounds and the
    /// like lays out again only when one came since its value was worked
    /// out.
    pub gui_epoch: u64,
    /// The screen size in dp the GUI was last laid out for.
    pub gui_view: (f32, f32),
    /// Layouts a read made, and the nodes they laid out in all.
    pub fresh_layouts: u64,
    pub fresh_nodes: u64,
    /// UIPageLayouts turning to a page.
    pub page_turns: Vec<super::hstudio_rbx_draw::PageTurn>,
    /// The GUIs in StarterGui already copied into the PlayerGui.
    pub starter_copied: HashSet<Id>,
}

/// An HTTP request a thread waits for.
pub struct HttpWait {
    pub owner: Owner,
    pub thread: Rc<Thread>,
    pub pending: super::hstudio_net::Pending,
}

/// A WebSocket: its Instance, the socket, and the thread waiting for it to open.
pub struct SocketEntry {
    pub id: Id,
    pub owner: Owner,
    pub socket: super::hstudio_net::Socket,
    pub opening: Option<Rc<Thread>>,
}

/// A userdata of `data` with `meta`.
fn ud<T: 'static>(data: T, meta: &TableRef) -> Value {
    Userdata::value(data, Some(meta.clone()))
}

fn host_meta(type_name: &str, entries: &[(&str, u32)]) -> TableRef {
    let mut t = Table::new();
    t.set_type_name(type_name);
    for (k, id) in entries {
        t.set_str(k, Native::host(k, *id));
    }
    t.set_str("__metatable", Value::str("The metatable is locked"));
    Rc::new(RefCell::new(t))
}

impl Default for Rbx {
    fn default() -> Self {
        Rbx::new()
    }
}

impl Rbx {
    pub fn new() -> Rbx {
        let inst_meta = host_meta(
            "Instance",
            &[
                ("__index", INST_INDEX),
                ("__newindex", INST_NEWINDEX),
                ("__tostring", INST_TOSTRING),
            ],
        );
        let signal_meta = host_meta(
            "RBXScriptSignal",
            &[("__index", SIGNAL_INDEX), ("__tostring", SIGNAL_TOSTRING)],
        );
        let conn_meta = host_meta(
            "RBXScriptConnection",
            &[("__index", CONN_INDEX), ("__tostring", SIGNAL_TOSTRING)],
        );
        let mut r = Rbx {
            insts: HashMap::new(),
            next_id: 1,
            next_conn: 1,
            conn_count: 0,
            game: 0,
            workspace: 0,
            players: 0,
            local_player: 0,
            player_gui: 0,
            core_gui: 0,
            camera: 0,
            character: 0,
            root_part: 0,
            head: 0,
            humanoid: 0,
            mouse: 0,
            services: HashMap::new(),
            inst_meta,
            signal_meta,
            conn_meta,
            natives: HashMap::new(),
            deferred: Vec::new(),
            ready: Vec::new(),
            waiters: Vec::new(),
            tweens: Vec::new(),
            debris: Vec::new(),
            actions: Vec::new(),
            render_steps: Vec::new(),
            core_gui_off: HashSet::new(),
            pointer: Pointer::default(),
            warnings: Vec::new(),
            sends: Vec::new(),
            sounds: Vec::new(),
            agent: Vec::new(),
            now: 0.0,
            layout: None,
            last_health: None,
            active: false,
            hui: 0,
            http: Vec::new(),
            sockets: Vec::new(),
            drawings: Vec::new(),
            next_drawing: 1,
            lua_methods: Vec::new(),
            loaded_modules: Vec::new(),
            workspace_dir: std::path::PathBuf::new(),
            screen_images: super::hstudio_sunc::ImageCache::default(),
            gui_epoch: 1,
            gui_view: (0.0, 0.0),
            fresh_layouts: 0,
            fresh_nodes: 0,
            page_turns: Vec::new(),
            starter_copied: HashSet::new(),
        };
        r.build();
        r
    }

    /// The DataModel, its services and the local player's things.
    fn build(&mut self) {
        let game = self.make("DataModel", None);
        self.game = game;
        self.inst_mut(game).name = "Game".into();
        self.lock(game);
        for c in CLASSES.iter().filter(|c| c.service) {
            let s = self.make(c.name, None);
            self.inst_mut(s).name = c.name.into();
            self.services.insert(c.name, s);
            let _ = self.set_parent(s, Some(game));
            self.lock(s);
        }
        self.workspace = self.services["Workspace"];
        self.players = self.services["Players"];
        self.core_gui = self.services["CoreGui"];
        let camera = self.make("Camera", None);
        self.camera = camera;
        let _ = self.set_parent(camera, Some(self.workspace));
        let terrain = self.make("Terrain", None);
        let _ = self.set_parent(terrain, Some(self.workspace));
        self.lock(terrain);
        let ws = self.workspace;
        let cam_v = self.value(camera);
        let ter_v = self.value(terrain);
        self.raw_set(ws, "CurrentCamera", cam_v);
        self.raw_set(ws, "Terrain", ter_v);
        let player = self.make("Player", None);
        self.inst_mut(player).name = "Player1".into();
        self.local_player = player;
        let _ = self.set_parent(player, Some(self.players));
        self.lock(player);
        let pv = self.value(player);
        let players = self.players;
        self.raw_set(players, "LocalPlayer", pv);
        for kind in ["PlayerGui", "PlayerScripts", "Backpack"] {
            let c = self.make(kind, None);
            self.inst_mut(c).name = kind.into();
            let _ = self.set_parent(c, Some(player));
            self.lock(c);
            if kind == "PlayerGui" {
                self.player_gui = c;
            }
        }
        // The character: a model with a root part, a head and a humanoid.
        let character = self.make("Model", None);
        self.inst_mut(character).name = "Player1".into();
        self.character = character;
        for (name, size) in [("HumanoidRootPart", (2.0, 2.0, 1.0)), ("Head", (2.0, 1.0, 1.0))] {
            let part = self.make("Part", None);
            self.inst_mut(part).name = name.into();
            self.raw_set(part, "Size", v3(Vector3::new(size.0, size.1, size.2)));
            self.raw_set(part, "Transparency", Value::Num(1.0));
            self.raw_set(part, "Anchored", Value::Bool(true));
            let _ = self.set_parent(part, Some(character));
            if name == "HumanoidRootPart" {
                self.root_part = part;
            } else {
                self.head = part;
            }
        }
        let humanoid = self.make("Humanoid", None);
        self.humanoid = humanoid;
        let root_v = self.value(self.root_part);
        self.raw_set(humanoid, "RootPart", root_v);
        self.raw_set(humanoid, "DisplayName", Value::str("Player1"));
        let _ = self.set_parent(humanoid, Some(character));
        let root_v = self.value(self.root_part);
        self.raw_set(character, "PrimaryPart", root_v);
        let _ = self.set_parent(character, Some(self.workspace));
        let cv = self.value(character);
        self.raw_set(player, "Character", cv);
        let sub = self.value(humanoid);
        self.raw_set(camera, "CameraSubject", sub);
        let mouse = self.make("PlayerMouse", None);
        self.inst_mut(mouse).name = "Instance".into();
        self.mouse = mouse;
        for id in [character, self.root_part, self.head, humanoid, camera, mouse] {
            self.lock(id);
        }
        let hui = self.make("Folder", None);
        self.inst_mut(hui).name = "HiddenUI".into();
        self.hui = hui;
        // Roblox's Instance metatable is locked.
        self.inst_meta.borrow_mut().set_readonly(true);
    }

    fn lock(&mut self, id: Id) {
        self.inst_mut(id).locked = true;
    }

    /// A new instance of `class` (which must be in CLASSES).
    pub fn make(&mut self, class: &str, owner: Option<Owner>) -> Id {
        let c = class_def(class).unwrap_or_else(|| panic!("no class {class}"));
        let id = self.next_id;
        self.next_id += 1;
        let mut props = HashMap::new();
        for p in all_props(c) {
            props.insert(p.name, prop_default(p));
        }
        let name = match c.name {
            "PlayerMouse" => "Instance",
            n => n,
        };
        let value = ud(InstRef(id), &self.inst_meta);
        self.insts.insert(
            id,
            Inst {
                id,
                class: c,
                name: name.to_owned(),
                parent: None,
                children: Vec::new(),
                props,
                extra: HashMap::new(),
                attributes: Vec::new(),
                tags: Vec::new(),
                conns: Vec::new(),
                value,
                owner,
                destroyed: false,
                locked: false,
                unscriptable: HashSet::new(),
                revealed: HashSet::new(),
                gui: is_a(c, "GuiBase2d") || is_a(c, "UIComponent"),
                laid_out: 0,
                placed_at: 0,
            },
        );
        id
    }

    pub fn inst(&self, id: Id) -> &Inst {
        &self.insts[&id]
    }
    pub fn inst_mut(&mut self, id: Id) -> &mut Inst {
        self.insts.get_mut(&id).expect("a live instance")
    }
    pub fn try_inst(&self, id: Id) -> Option<&Inst> {
        self.insts.get(&id)
    }
    pub fn value(&self, id: Id) -> Value {
        self.insts.get(&id).map_or(Value::Nil, |i| i.value.clone())
    }
    pub fn service(&self, name: &str) -> Option<Id> {
        self.services.get(name).copied()
    }

    /// The instance a value is, if it is one.
    pub fn id_of(&self, v: &Value) -> Option<Id> {
        match v {
            Value::Userdata(u) => {
                let d = u.data.borrow();
                let r = d.downcast_ref::<InstRef>()?;
                self.insts.contains_key(&r.0).then_some(r.0)
            }
            Value::Table(_) => {
                let g = self.insts.get(&self.game)?;
                v.raw_eq(&g.value).then_some(self.game)
            }
            _ => None,
        }
    }

    /// Makes the shared `game` table the DataModel's value: it keeps the
    /// studio's `game.*` functions and gets the DataModel's members too.
    pub fn adopt_game_table(&mut self, t: &TableRef) {
        // The same metatable as every Instance's, as in Roblox.
        t.borrow_mut().meta = Some(self.inst_meta.clone());
        let game = self.game;
        self.inst_mut(game).value = Value::Table(t.clone());
    }

    /// A property's value, without metamethods or checks.
    pub fn raw_get(&self, id: Id, name: &str) -> Value {
        self.insts.get(&id).map(|i| i.get(name)).unwrap_or_default()
    }
    /// Sets a property, firing Changed if it changed.
    pub fn raw_set(&mut self, id: Id, name: &'static str, v: Value) {
        let Some(inst) = self.insts.get_mut(&id) else {
            return;
        };
        let old = inst.props.insert(name, v.clone()).unwrap_or_default();
        if !same_value(&old, &v) {
            self.changed(id, name, &v);
        }
    }
    /// Sets a property quietly (the layer's own bookkeeping, as AbsoluteSize).
    pub fn quiet_set(&mut self, id: Id, name: &'static str, v: Value) {
        if let Some(inst) = self.insts.get_mut(&id) {
            inst.props.insert(name, v);
        }
    }

    /// Changed and the property's own signal, as Roblox fires them (a
    /// ValueBase's Changed is for its Value only, with the value).
    pub fn changed(&mut self, id: Id, name: &str, v: &Value) {
        // Every change the layout can see goes through here (properties,
        // Parent, Name), except the layout's own outputs and what only the
        // drawing reads. A Parent counts when a GUI instance is in what
        // moved: a Folder of GuiObjects moves them, and a part a
        // BillboardGui is in moves it.
        let gui = self.insts.get(&id).is_some_and(|i| i.gui);
        let moves = match name {
            "Parent" => self.holds_gui(id),
            _ => {
                gui && !super::hstudio_rbx_draw::OUTPUTS.contains(&name)
                    && !super::hstudio_rbx_draw::DRAWN.contains(&name)
            }
        };
        if moves {
            self.gui_epoch += 1;
        }
        let value_base = self.insts.get(&id).is_some_and(|i| is_a(i.class, "ValueBase"));
        if value_base && name == "Value" {
            self.fire(id, "Changed", vec![v.clone()]);
        } else if !value_base {
            self.fire(id, "Changed", vec![Value::str(name)]);
        }
        self.fire(id, &format!("prop:{name}"), Vec::new());
        self.derived_changed(id, name);
    }

    /// The signals of the read-only properties made of `name`: ContentText
    /// follows Text and RichText, GuiState follows Interactable.
    fn derived_changed(&mut self, id: Id, name: &str) {
        let Some(c) = self.insts.get(&id).map(|i| i.class) else {
            return;
        };
        if matches!(name, "Text" | "RichText") && find_prop(c, "RichText").is_some() {
            self.fire(id, "prop:ContentText", Vec::new());
        }
        if name == "Interactable" && is_a(c, "GuiObject") {
            self.fire(id, "prop:GuiState", Vec::new());
        }
    }

    // ------------------------------------------------------------- the tree

    pub fn children(&self, id: Id) -> &[Id] {
        self.insts.get(&id).map_or(&[], |i| &i.children)
    }

    pub fn descendants(&self, id: Id) -> Vec<Id> {
        let mut out = Vec::new();
        let mut stack: Vec<Id> = self.children(id).iter().rev().copied().collect();
        while let Some(n) = stack.pop() {
            out.push(n);
            stack.extend(self.children(n).iter().rev().copied());
        }
        out
    }

    /// Whether `id` or something under it is a GUI instance.
    pub fn holds_gui(&self, id: Id) -> bool {
        let mut stack = vec![id];
        while let Some(n) = stack.pop() {
            let Some(inst) = self.insts.get(&n) else { continue };
            if inst.gui {
                return true;
            }
            stack.extend(inst.children.iter().copied());
        }
        false
    }

    pub fn is_descendant_of(&self, id: Id, ancestor: Id) -> bool {
        let mut at = self.insts.get(&id).and_then(|i| i.parent);
        while let Some(p) = at {
            if p == ancestor {
                return true;
            }
            at = self.insts.get(&p).and_then(|i| i.parent);
        }
        false
    }

    pub fn full_name(&self, id: Id) -> String {
        let mut parts = Vec::new();
        let mut at = Some(id);
        while let Some(i) = at {
            if i == self.game {
                break;
            }
            let Some(inst) = self.insts.get(&i) else { break };
            parts.push(inst.name.clone());
            at = inst.parent;
        }
        parts.reverse();
        parts.join(".")
    }

    /// Moves `id` under `parent`, with the events Roblox fires.
    pub fn set_parent(&mut self, id: Id, parent: Option<Id>) -> Result<(), String> {
        let (old, name, locked, destroyed) = {
            let i = self.inst(id);
            (i.parent, i.full_name_hint(), i.locked, i.destroyed)
        };
        if old == parent {
            return Ok(());
        }
        if destroyed || locked && old.is_some() {
            return Err(format!(
                "The Parent property of {name} is locked, current parent: {}, new parent {}",
                old.map_or("NULL".into(), |p| self.inst(p).name.clone()),
                parent.map_or("NULL".into(), |p| self.inst(p).name.clone())
            ));
        }
        if let Some(p) = parent {
            if p == id || self.is_descendant_of(p, id) {
                return Err(format!(
                    "Attempt to set parent of {} to {} would result in circular reference",
                    self.full_name(id),
                    self.full_name(p)
                ));
            }
            if self.inst(p).destroyed {
                return Err(format!(
                    "The Parent property of {name} is locked (its new parent was destroyed)"
                ));
            }
        }
        if let Some(o) = old {
            let mut removed = vec![id];
            removed.extend(self.descendants(id));
            let mut up = Some(o);
            while let Some(a) = up {
                for &d in &removed {
                    let dv = self.value(d);
                    self.fire(a, "DescendantRemoving", vec![dv]);
                }
                up = self.inst(a).parent;
            }
            self.inst_mut(o).children.retain(|&c| c != id);
            let v = self.value(id);
            self.fire(o, "ChildRemoved", vec![v]);
        }
        self.inst_mut(id).parent = parent;
        if let Some(p) = parent {
            self.inst_mut(p).children.push(id);
            let v = self.value(id);
            self.fire(p, "ChildAdded", vec![v]);
            let mut added = vec![id];
            added.extend(self.descendants(id));
            let mut up = Some(p);
            while let Some(a) = up {
                for &d in &added {
                    let dv = self.value(d);
                    self.fire(a, "DescendantAdded", vec![dv]);
                }
                up = self.inst(a).parent;
            }
            self.child_arrived(p, id);
        }
        let pv = parent.map_or(Value::Nil, |p| self.value(p));
        let me = self.value(id);
        self.fire(id, "AncestryChanged", vec![me.clone(), pv.clone()]);
        for d in self.descendants(id) {
            self.fire(d, "AncestryChanged", vec![me.clone(), pv.clone()]);
        }
        self.changed(id, "Parent", &Value::Nil);
        Ok(())
    }

    /// Destroys `id` and everything under it, as Instance:Destroy.
    pub fn destroy(&mut self, id: Id) {
        if self.insts.get(&id).is_none_or(|i| i.destroyed) {
            return;
        }
        self.fire(id, "Destroying", Vec::new());
        // Destroying's handlers are deferred: they still get the instance.
        if self.inst(id).parent.is_some() {
            self.inst_mut(id).locked = false;
            let _ = self.set_parent(id, None);
        }
        let kids = self.inst(id).children.clone();
        for c in kids {
            self.destroy(c);
        }
        let inst = self.inst_mut(id);
        inst.destroyed = true;
        inst.locked = true;
        let n = inst.conns.len();
        inst.conns.clear();
        self.conn_count -= n;
        self.tweens.retain(|t| t.target != id && t.id != id);
        self.waiters.retain(|w| match &w.what {
            Wait::Signal(i, _) => *i != id,
            Wait::Child { parent, .. } => *parent != id,
        });
    }

    /// Drops everything `owner` made: its instances, connections, tweens,
    /// waiting threads, actions and render steps.
    pub fn forget(&mut self, owner: Owner) {
        let mine: Vec<Id> = self
            .insts
            .values()
            .filter(|i| i.owner == Some(owner) && !i.destroyed)
            .map(|i| i.id)
            .collect();
        for id in mine {
            self.destroy(id);
        }
        let mut removed = 0;
        for inst in self.insts.values_mut() {
            let before = inst.conns.len();
            inst.conns.retain(|c| c.owner != owner);
            removed += before - inst.conns.len();
        }
        self.conn_count -= removed;
        // Destroyed instances nothing can reach any more are let go, and so
        // is what require kept for those that were ModuleScripts.
        self.insts.retain(|_, i| !(i.destroyed && i.owner == Some(owner)));
        let insts = &self.insts;
        self.loaded_modules.retain(|(id, _)| insts.contains_key(id));
        self.tweens.retain(|t| t.owner != owner);
        self.waiters.retain(|w| w.owner != owner);
        self.deferred.retain(|d| d.0 != owner);
        self.ready.retain(|r| r.0 != owner);
        self.actions.retain(|a| a.1 != owner);
        self.render_steps.retain(|r| r.2 != owner);
        self.http.retain(|h| h.owner != owner);
        for s in self.sockets.iter().filter(|s| s.owner == owner) {
            s.socket.close();
        }
        self.sockets.retain(|s| s.owner != owner);
        self.drawings.retain(|d| d.owner != owner);
        if self.pointer.focus.is_some_and(|f| !self.insts.contains_key(&f)) {
            self.pointer.focus = None;
        }
        self.pointer.hover.retain(|h| self.insts.contains_key(h));
        self.pointer.pressed = None;
        self.pointer.drag = None;
        self.layout = None;
    }

    /// A child named `name` of `parent`, the first.
    pub fn child_named(&self, parent: Id, name: &str) -> Option<Id> {
        self.children(parent)
            .iter()
            .copied()
            .find(|&c| self.inst(c).name == name)
    }

    fn child_arrived(&mut self, parent: Id, child: Id) {
        let name = self.inst(child).name.clone();
        let cv = self.value(child);
        let mut i = 0;
        while i < self.waiters.len() {
            let hit =
                matches!(&self.waiters[i].what, Wait::Child { parent: p, name: n, .. } if *p == parent && *n == name);
            if hit {
                let w = self.waiters.remove(i);
                self.ready.push((w.owner, w.thread, vec![cv.clone()]));
            } else {
                i += 1;
            }
        }
    }

    // ------------------------------------------------------------- signals

    /// Fires `event` of `id`: its handlers run deferred, its waiters resume.
    pub fn fire(&mut self, id: Id, event: &str, args: Vec<Value>) {
        let Some(inst) = self.insts.get_mut(&id) else {
            return;
        };
        if !inst.conns.is_empty() {
            let mut fired_once = 0;
            for c in inst.conns.iter().filter(|c| &*c.event == event && c.enabled) {
                self.deferred.push((c.owner, c.f.clone(), args.clone()));
                if c.once {
                    fired_once += 1;
                }
            }
            if fired_once > 0 {
                inst.conns.retain(|c| !(c.once && c.enabled && &*c.event == event));
                self.conn_count -= fired_once;
            }
        }
        if !self.waiters.is_empty() {
            let mut i = 0;
            while i < self.waiters.len() {
                let hit = matches!(&self.waiters[i].what, Wait::Signal(w, e) if *w == id && &**e == event);
                if hit {
                    let w = self.waiters.remove(i);
                    self.ready.push((w.owner, w.thread, args.clone()));
                } else {
                    i += 1;
                }
            }
        }
    }

    fn connect(
        &mut self,
        id: Id,
        event: Rc<str>,
        owner: Owner,
        f: Value,
        once: bool,
        thread: Option<Rc<Thread>>,
    ) -> Result<u32, String> {
        if self.conn_count >= MAX_CONNECTIONS {
            return Err(format!("too many connections ({MAX_CONNECTIONS})"));
        }
        let cid = self.next_conn;
        self.next_conn += 1;
        self.conn_count += 1;
        self.inst_mut(id).conns.push(Conn {
            id: cid,
            event,
            owner,
            f,
            once,
            enabled: true,
            thread,
        });
        Ok(cid)
    }

    pub fn signal(&self, id: Id, event: &str) -> Value {
        ud(
            SignalRef {
                inst: id,
                event: event.into(),
            },
            &self.signal_meta,
        )
    }

    /// The value of a host function, made once.
    fn native(&mut self, id: u32, name: &str) -> Value {
        self.natives.entry(id).or_insert_with(|| Native::host(name, id)).clone()
    }
}

impl Inst {
    fn full_name_hint(&self) -> String {
        self.name.clone()
    }
}

/// Whether two property values are the same (for Changed).
pub fn same_value(a: &Value, b: &Value) -> bool {
    if a.raw_eq(b) {
        return true;
    }
    match (get(a), get(b)) {
        (Some(x), Some(y)) => values::same(&x, &y),
        _ => false,
    }
}

// ---------------------------------------------------------------- scripts' calls

/// What the layer needs of the studio while a script runs: the running
/// call's owner, the clock and the game.
pub trait Env: Host {
    fn rbx(&mut self) -> &mut Rbx;
    fn owner(&self) -> Owner;
    fn now(&self) -> f64;
    fn game(&mut self) -> &mut dyn GameApi;
    /// The layer and the game at once (they are separate parts of the studio).
    fn rbx_and_game(&mut self) -> (&mut Rbx, &mut dyn GameApi);
    /// Whether a key ("key.keyboard.e") is held.
    fn held(&self, key: &str) -> bool;
    /// Runs `f` with `args` now in a thread of its own, as task.spawn does.
    fn spawn_now(&mut self, lua: &mut Lua, f: Value, args: Vec<Value>);
}

/// Methods, each with the class that has it.
const METHODS: &[(&str, &str)] = &[
    ("FindFirstChild", "Instance"),
    ("FindFirstChildOfClass", "Instance"),
    ("FindFirstChildWhichIsA", "Instance"),
    ("FindFirstAncestor", "Instance"),
    ("FindFirstAncestorOfClass", "Instance"),
    ("FindFirstAncestorWhichIsA", "Instance"),
    ("FindFirstDescendant", "Instance"),
    ("GetChildren", "Instance"),
    ("GetDescendants", "Instance"),
    ("IsA", "Instance"),
    ("IsDescendantOf", "Instance"),
    ("IsAncestorOf", "Instance"),
    ("GetFullName", "Instance"),
    ("Destroy", "Instance"),
    ("Remove", "Instance"),
    ("ClearAllChildren", "Instance"),
    ("Clone", "Instance"),
    ("WaitForChild", "Instance"),
    ("GetPropertyChangedSignal", "Instance"),
    ("GetAttribute", "Instance"),
    ("SetAttribute", "Instance"),
    ("GetAttributes", "Instance"),
    ("GetAttributeChangedSignal", "Instance"),
    ("AddTag", "Instance"),
    ("RemoveTag", "Instance"),
    ("HasTag", "Instance"),
    ("GetTags", "Instance"),
    ("GetDebugId", "Instance"),
    ("GetActor", "Instance"),
    ("GetService", "DataModel"),
    ("Send", "WebSocket"),
    ("Close", "WebSocket"),
    ("FindService", "DataModel"),
    ("IsLoaded", "DataModel"),
    ("BindToClose", "DataModel"),
    ("GetPlayers", "Players"),
    ("GetPlayerFromCharacter", "Players"),
    ("GetPlayerByUserId", "Players"),
    ("GetNameFromUserIdAsync", "Players"),
    ("GetUserIdFromNameAsync", "Players"),
    ("GetUserThumbnailAsync", "Players"),
    ("GetMouse", "Player"),
    ("Kick", "Player"),
    ("IsFriendsWith", "Player"),
    ("IsInGroup", "Player"),
    ("GetRankInGroup", "Player"),
    ("LoadCharacter", "Player"),
    ("DistanceFromCharacter", "Player"),
    ("IsClient", "RunService"),
    ("IsServer", "RunService"),
    ("IsStudio", "RunService"),
    ("IsRunning", "RunService"),
    ("IsRunMode", "RunService"),
    ("BindToRenderStep", "RunService"),
    ("UnbindFromRenderStep", "RunService"),
    ("IsKeyDown", "UserInputService"),
    ("IsMouseButtonPressed", "UserInputService"),
    ("GetMouseLocation", "UserInputService"),
    ("GetKeysPressed", "UserInputService"),
    ("GetMouseButtonsPressed", "UserInputService"),
    ("GetFocusedTextBox", "UserInputService"),
    ("GetMouseDelta", "UserInputService"),
    ("GetLastInputType", "UserInputService"),
    ("GetStringForKeyCode", "UserInputService"),
    ("Create", "TweenService"),
    ("GetValue", "TweenService"),
    ("Play", "Tween"),
    ("Pause", "Tween"),
    ("Cancel", "Tween"),
    ("AddItem", "Debris"),
    ("JSONEncode", "HttpService"),
    ("JSONDecode", "HttpService"),
    ("GenerateGUID", "HttpService"),
    ("UrlEncode", "HttpService"),
    ("GetAsync", "HttpService"),
    ("PostAsync", "HttpService"),
    ("RequestAsync", "HttpService"),
    ("GetTextSize", "TextService"),
    ("GetTextBoundsAsync", "TextService"),
    ("GetGuiInset", "GuiService"),
    ("IsTenFootInterface", "GuiService"),
    ("SetCore", "StarterGui"),
    ("GetCore", "StarterGui"),
    ("SetCoreGuiEnabled", "StarterGui"),
    ("GetCoreGuiEnabled", "StarterGui"),
    ("GetTagged", "CollectionService"),
    ("GetInstanceAddedSignal", "CollectionService"),
    ("GetInstanceRemovedSignal", "CollectionService"),
    ("BindAction", "ContextActionService"),
    ("BindActionAtPriority", "ContextActionService"),
    ("UnbindAction", "ContextActionService"),
    ("UnbindAllActions", "ContextActionService"),
    ("Raycast", "Workspace"),
    ("GetServerTimeNow", "Workspace"),
    ("WorldToViewportPoint", "Camera"),
    ("WorldToScreenPoint", "Camera"),
    ("ViewportPointToRay", "Camera"),
    ("ScreenPointToRay", "Camera"),
    ("GetPivot", "PVInstance"),
    ("PivotTo", "PVInstance"),
    ("MoveTo", "Model"),
    ("SetPrimaryPartCFrame", "Model"),
    ("GetPrimaryPartCFrame", "Model"),
    ("GetMass", "BasePart"),
    ("GetTouchingParts", "BasePart"),
    ("MoveTo", "Humanoid"),
    ("TakeDamage", "Humanoid"),
    ("ChangeState", "Humanoid"),
    ("GetState", "Humanoid"),
    ("UnequipTools", "Humanoid"),
    ("GetPlayingAnimationTracks", "Humanoid"),
    ("Fire", "BindableEvent"),
    ("Invoke", "BindableFunction"),
    ("Play", "Sound"),
    ("Play", "VideoFrame"),
    ("Pause", "VideoFrame"),
    ("Stop", "Sound"),
    ("Pause", "Sound"),
    ("Resume", "Sound"),
    ("CaptureFocus", "TextBox"),
    ("ReleaseFocus", "TextBox"),
    ("IsFocused", "TextBox"),
    ("TweenPosition", "GuiObject"),
    ("TweenSize", "GuiObject"),
    ("TweenSizeAndPosition", "GuiObject"),
    ("GetGuiObjectsAtPosition", "PlayerGui"),
    ("GetGuiObjectsAtPosition", "LayerCollector"),
    ("JumpTo", "UIPageLayout"),
    ("JumpToIndex", "UIPageLayout"),
    ("Next", "UIPageLayout"),
    ("Previous", "UIPageLayout"),
];

const SIGNAL_METHODS: &[&str] = &["Connect", "Once", "Wait", "ConnectParallel"];

/// The method of `class` called `name` (also its older lowercase name).
fn find_method(c: &'static C, name: &str) -> Option<usize> {
    let pick = |n: &str| METHODS.iter().position(|(m, owner)| *m == n && is_a(c, owner));
    pick(name).or_else(|| {
        let mut chars = name.chars();
        let first = chars.next()?;
        if !first.is_ascii_lowercase() {
            return None;
        }
        pick(&format!("{}{}", first.to_ascii_uppercase(), chars.as_str()))
    })
}

fn kind_name(k: K) -> String {
    match k {
        K::Bool => "bool".into(),
        K::Num | K::Int => "number".into(),
        K::Str => "string".into(),
        K::V2 => "Vector2".into(),
        K::V3 => "Vector3".into(),
        K::CF => "CFrame".into(),
        K::C3 => "Color3".into(),
        K::UD => "UDim".into(),
        K::UD2 => "UDim2".into(),
        K::Enum(e) => format!("Enum.{e}"),
        K::Obj => "Instance".into(),
        K::Font => "Font".into(),
        K::Brick => "BrickColor".into(),
        K::NumSeq => "NumberSequence".into(),
        K::ColorSeq => "ColorSequence".into(),
        K::Range => "NumberRange".into(),
        K::Rect => "Rect".into(),
        K::Func => "function".into(),
    }
}

/// `v` as a value of kind `k`, as Roblox converts on assignment.
fn coerce(rbx: &Rbx, k: K, v: &Value) -> Option<Value> {
    let d = get(v);
    Some(match k {
        K::Bool => match v {
            Value::Bool(_) => v.clone(),
            Value::Nil => Value::Bool(false),
            _ => return None,
        },
        K::Num => Value::Num(v.to_num()?),
        K::Int => Value::Num(v.to_num()?.floor()),
        K::Str => match v {
            Value::Str(_) => v.clone(),
            Value::Num(n) => Value::str(&super::hstudio_lua::fmt_num(*n)),
            _ => return None,
        },
        K::Enum(e) => enum_value(coerce_enum(e, v)?),
        K::Obj => match v {
            Value::Nil => Value::Nil,
            other => {
                rbx.id_of(other)?;
                other.clone()
            }
        },
        K::Func => match v {
            Value::Nil => Value::Nil,
            f if f.is_callable() => f.clone(),
            _ => return None,
        },
        K::NumSeq => match (v, d) {
            (Value::Num(n), _) => wrap(Data::NumSeq(NumberSequence::constant(*n))),
            (_, Some(Data::NumSeq(_))) => v.clone(),
            _ => return None,
        },
        K::ColorSeq => match d? {
            Data::ColorSeq(_) => v.clone(),
            Data::C3(c) => wrap(Data::ColorSeq(ColorSequence::constant(c))),
            _ => return None,
        },
        K::Range => match (v, d) {
            (Value::Num(n), _) => wrap(Data::Range(NumberRange::single(*n))),
            (_, Some(Data::Range(_))) => v.clone(),
            _ => return None,
        },
        _ => {
            let ok = matches!(
                (k, d?),
                (K::V2, Data::V2(_))
                    | (K::V3, Data::V3(_))
                    | (K::CF, Data::CF(_))
                    | (K::C3, Data::C3(_))
                    | (K::UD, Data::UD(_))
                    | (K::UD2, Data::UD2(_))
                    | (K::Font, Data::Font(_))
                    | (K::Brick, Data::Brick(_))
                    | (K::Rect, Data::Rect(_))
            );
            if !ok {
                return None;
            }
            v.clone()
        }
    })
}

fn deg(v: Vector3) -> Vector3 {
    Vector3::new(v.x.to_degrees(), v.y.to_degrees(), v.z.to_degrees())
}

/// The instance argument `i` (the `self` of a method is 0).
fn inst_arg(env: &mut dyn Env, lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<Id, Throw> {
    match args.get(i).and_then(|v| env.rbx().id_of(v)) {
        Some(id) => Ok(id),
        None => {
            let got = args.get(i).map_or("nil".to_owned(), |v| lua.type_of(v));
            if i == 0 {
                Err(lua.rt_error(format!("Expected ':' not '.' calling member function {fname}")))
            } else {
                Err(bad_arg(lua, i, fname, &format!("Instance expected, got {got}")))
            }
        }
    }
}

/// A host function of the layer.
pub fn call(env: &mut dyn Env, lua: &mut Lua, id: u32, args: Vec<Value>) -> Res {
    if !env.rbx().active {
        // The first use: the layer wakes up, with the game as it is now.
        let now = env.now();
        let (rbx, game) = env.rbx_and_game();
        rbx.active = true;
        rbx.now = now;
        super::hstudio_rbx_draw::sync(rbx, game);
    }
    match id {
        INST_INDEX => index(env, lua, &args),
        INST_NEWINDEX => {
            newindex(env, lua, &args)?;
            Ok(Vec::new())
        }
        INST_TOSTRING => {
            let id = inst_arg(env, lua, &args, 0, "tostring")?;
            Ok(vec![Value::str(&env.rbx().inst(id).name)])
        }
        SIGNAL_INDEX => {
            let k = args.get(1).and_then(Value::as_text).unwrap_or_default();
            let name = SIGNAL_METHODS
                .iter()
                .position(|m| m.eq_ignore_ascii_case(&k))
                .ok_or_else(|| lua.rt_error(format!("{k} is not a valid member of RBXScriptSignal")))?;
            Ok(vec![
                env.rbx().native(SIGNAL_METHOD_BASE + name as u32, SIGNAL_METHODS[name]),
            ])
        }
        CONN_INDEX => {
            let Some(c) = conn_ref(&args) else {
                return Err(lua.rt_error("bad connection"));
            };
            let k = args.get(1).and_then(Value::as_text).unwrap_or_default();
            match k.as_str() {
                "Connected" => {
                    let live = env
                        .rbx()
                        .try_inst(c.inst)
                        .is_some_and(|i| i.conns.iter().any(|x| x.id == c.id));
                    Ok(vec![Value::Bool(live)])
                }
                "Disconnect" | "disconnect" => Ok(vec![env.rbx().native(CONN_METHOD_BASE, "Disconnect")]),
                _ => Err(lua.rt_error(format!("{k} is not a valid member of RBXScriptConnection"))),
            }
        }
        SIGNAL_TOSTRING => Ok(vec![Value::str(match args.first() {
            Some(v) if conn_ref(&args).is_some() || lua.type_of(v) == "RBXScriptConnection" => "Connection",
            _ => "Signal",
        })]),
        INSTANCE_NEW => {
            let class = check_text(lua, &args, 0, "new")?;
            let c = class_def(&class)
                .filter(|c| c.creatable)
                .ok_or_else(|| lua.rt_error(format!("Unable to create an Instance of type \"{class}\"")))?;
            let owner = env.owner();
            let rbx = env.rbx();
            if rbx.insts.len() >= MAX_INSTANCES {
                return Err(lua.rt_error(format!("too many Instances ({MAX_INSTANCES})")));
            }
            let id = rbx.make(c.name, Some(owner));
            if let Some(p) = args.get(1).and_then(|v| rbx.id_of(v)) {
                rbx.set_parent(id, Some(p)).map_err(|e| lua.rt_error(e))?;
            }
            Ok(vec![rbx.value(id)])
        }
        INSTANCE_FROM_EXISTING => {
            let src = inst_arg(env, lua, &args, 1, "fromExisting")
                .or_else(|_| inst_arg(env, lua, &args, 0, "fromExisting"))?;
            let owner = env.owner();
            let rbx = env.rbx();
            let id = rbx.clone_tree(src, owner, false);
            Ok(vec![rbx.value(id)])
        }
        SCREEN_DRAW..=SCREEN_CLOSE | SCREEN_ESCAPE => {
            super::hstudio_rbx_draw::screen_call(env, lua, id - SCREEN_DRAW, &args)
        }
        CONN_METHOD_BASE => {
            if let Some(c) = conn_ref(&args) {
                let rbx = env.rbx();
                if let Some(inst) = rbx.insts.get_mut(&c.inst) {
                    let before = inst.conns.len();
                    inst.conns.retain(|x| x.id != c.id);
                    rbx.conn_count -= before - inst.conns.len();
                }
            }
            Ok(Vec::new())
        }
        _ if (SIGNAL_METHOD_BASE..SIGNAL_METHOD_BASE + SIGNAL_METHODS.len() as u32).contains(&id) => {
            signal_method(env, lua, SIGNAL_METHODS[(id - SIGNAL_METHOD_BASE) as usize], &args)
        }
        _ if (METHOD_BASE..METHOD_BASE + METHODS.len() as u32).contains(&id) => {
            let (name, owner_class) = METHODS[(id - METHOD_BASE) as usize];
            super::hstudio_rbx_methods::method(env, lua, name, owner_class, args)
        }
        _ => Err(lua.rt_error(format!("unknown Roblox function {id}"))),
    }
}

fn signal_ref(args: &[Value]) -> Option<SignalRef> {
    match args.first() {
        Some(Value::Userdata(u)) => u.data.borrow().downcast_ref::<SignalRef>().cloned(),
        _ => None,
    }
}

fn conn_ref(args: &[Value]) -> Option<ConnRef> {
    match args.first() {
        Some(Value::Userdata(u)) => u.data.borrow().downcast_ref::<ConnRef>().copied(),
        _ => None,
    }
}

fn signal_method(env: &mut dyn Env, lua: &mut Lua, name: &str, args: &[Value]) -> Res {
    let s =
        signal_ref(args).ok_or_else(|| lua.rt_error(format!("Expected ':' not '.' calling member function {name}")))?;
    if env.rbx().try_inst(s.inst).is_none() {
        return Err(lua.rt_error("the signal's Instance is gone"));
    }
    match name {
        "Wait" => {
            if !lua.is_yieldable() {
                return Err(lua.rt_error(
                    "Wait can't wait here: this code isn't in a thread that can wait (run it with task.spawn)",
                ));
            }
            let owner = env.owner();
            let thread = lua.current_thread();
            env.rbx().waiters.push(Waiter {
                owner,
                thread,
                what: Wait::Signal(s.inst, s.event.clone()),
            });
            Err(Throw::Yield(Vec::new()))
        }
        _ => {
            let f = args
                .get(1)
                .filter(|f| f.is_callable())
                .cloned()
                .ok_or_else(|| bad_arg(lua, 1, name, "function expected"))?;
            let owner = env.owner();
            let rbx = env.rbx();
            let thread = Some(lua.current_thread());
            let cid = rbx
                .connect(s.inst, s.event.clone(), owner, f, name == "Once", thread)
                .map_err(|e| lua.rt_error(e))?;
            let c = ud(ConnRef { inst: s.inst, id: cid }, &rbx.conn_meta);
            Ok(vec![c])
        }
    }
}

/// `inst.key`.
fn index(env: &mut dyn Env, lua: &mut Lua, args: &[Value]) -> Res {
    let id = inst_arg(env, lua, args, 0, "index")?;
    let key = args.get(1).cloned().unwrap_or_default();
    let Some(k) = key.as_text() else {
        return Err(lua.rt_error(format!(
            "{} is not a valid member of {}",
            key.plain_text(),
            env.rbx().inst(id).class.name
        )));
    };
    if id == env.rbx().mouse && matches!(k.as_str(), "Hit" | "Target" | "UnitRay") {
        super::hstudio_rbx_draw::mouse_refresh(env);
    }
    if matches!(k.as_str(), "ViewportSize" | "ViewSizeX" | "ViewSizeY") {
        let snap = env_snapshot(env);
        env.rbx().refresh(id, &k, snap.as_ref());
    }
    if k == "IsLoaded"
        && let Some(loaded) = env.rbx().image_loaded(id)
    {
        return Ok(vec![Value::Bool(loaded)]);
    }
    // AbsoluteSize and the layout's other outputs are worked out afresh
    // when the GUI changed since; the nodes laid out count as steps.
    if super::hstudio_rbx_draw::OUTPUTS.contains(&k.as_str()) && find_prop(env.rbx().inst(id).class, &k).is_some() {
        let (rbx, game) = env.rbx_and_game();
        let nodes = super::hstudio_rbx_draw::fresh_output(rbx, game, id, &k);
        lua.charge(nodes * 16)?;
    }
    let rbx = env.rbx();
    let inst = rbx.inst(id);
    let c = inst.class;
    if inst.unscriptable.contains(&k) {
        return Err(lua.rt_error(format!(
            "{k} is not a valid member of {} \"{}\"",
            c.name,
            rbx.full_name(id)
        )));
    }
    let one = |v: Value| Ok(vec![v]);
    match k.as_str() {
        "Name" => return one(Value::str(&inst.name)),
        "ClassName" => return one(Value::str(c.name)),
        "Parent" => return one(inst.parent.map_or(Value::Nil, |p| rbx.value(p))),
        _ => {}
    }
    if let Some(p) = find_prop(c, &k) {
        return one(inst.get(p.name));
    }
    // Properties made of others.
    let derived = match k.as_str() {
        "Position" | "Orientation" | "Rotation" if is_a(c, "BasePart") => {
            let frame = as_cf(&inst.get("CFrame")).unwrap_or_default();
            Some(match k.as_str() {
                "Position" => v3(frame.position()),
                "Orientation" => {
                    let (x, y, z) = frame.to_orientation();
                    v3(deg(Vector3::new(x, y, z)))
                }
                _ => {
                    let (x, y, z) = frame.to_euler_angles_xyz();
                    v3(deg(Vector3::new(x, y, z)))
                }
            })
        }
        "BrickColor" if is_a(c, "BasePart") => Some(wrap(Data::Brick(BrickColor::from_color3(
            as_c3(&inst.get("Color")).unwrap_or_default(),
        )))),
        "Font" if find_prop(c, "FontFace").is_some() => {
            let face = as_font(&inst.get("FontFace")).unwrap_or_default();
            Some(enum_value(enum_item("Font", face.to_enum().name())))
        }
        "Value" if is_a(c, "ValueBase") => Some(inst.get("Value")),
        "Focus" | "CFrame" if c.name == "Camera" => Some(inst.get(&k)),
        // The text as drawn: Text without rich text's tags.
        "ContentText" if find_prop(c, "RichText").is_some() => {
            let text = inst.get("Text").as_text().unwrap_or_default();
            Some(Value::str(&if inst.get("RichText").truthy() {
                super::hstudio_rbx_draw::strip_tags(&text)
            } else {
                text
            }))
        }
        "GuiState" if is_a(c, "GuiObject") => Some(enum_value(enum_item(
            "GuiState",
            super::hstudio_rbx_draw::gui_state(rbx, id),
        ))),
        // GuiObject's deprecated Transparency is its BackgroundTransparency.
        "Transparency" if is_a(c, "GuiObject") => Some(inst.get("BackgroundTransparency")),
        _ => None,
    };
    if let Some(v) = derived {
        return one(v);
    }
    if (c.lenient || inst.revealed.contains(&k))
        && let Some(v) = inst.extra.get(&k)
    {
        return one(v.clone());
    }
    if has_event(c, &k) {
        return one(rbx.signal(id, &k));
    }
    if let Some((_, _, f)) = rbx.lua_methods.iter().find(|(cl, n, _)| *n == k && is_a(c, cl)) {
        return one(f.clone());
    }
    if let Some(m) = find_method(c, &k) {
        return one(rbx.native(METHOD_BASE + m as u32, METHODS[m].0));
    }
    if let Some(child) = rbx.child_named(id, &k) {
        return one(rbx.value(child));
    }
    // game.Players and the like: a service by its name.
    if id == rbx.game
        && let Some(s) = rbx.service(&k)
    {
        return one(rbx.value(s));
    }
    if c.lenient {
        return one(Value::Nil);
    }
    Err(lua.rt_error(format!(
        "{k} is not a valid member of {} \"{}\"",
        c.name,
        rbx.full_name(id)
    )))
}

/// What a read of the live player or camera needs from the game, taken
/// before the layer is borrowed.
pub struct Snapshot {
    pub view: (f64, f64),
}

fn env_snapshot(env: &mut dyn Env) -> Option<Snapshot> {
    let (w, h, _) = env.game().dp();
    Some(Snapshot {
        view: (f64::from(w), f64::from(h)),
    })
}

impl Rbx {
    /// Brings a property the game decides up to date before it is read.
    fn refresh(&mut self, id: Id, key: &str, snap: Option<&Snapshot>) {
        let Some(snap) = snap else { return };
        if id == self.camera && key == "ViewportSize" {
            self.quiet_set(id, "ViewportSize", v2(Vector2::new(snap.view.0, snap.view.1)));
        }
        if id == self.mouse && matches!(key, "ViewSizeX" | "ViewSizeY") {
            self.quiet_set(id, "ViewSizeX", Value::Num(snap.view.0));
            self.quiet_set(id, "ViewSizeY", Value::Num(snap.view.1));
        }
    }

    /// An image object's IsLoaded: whether its Image draws, which only a
    /// workspace file (getcustomasset's) that is a PNG does. None for an
    /// instance without an Image.
    pub fn image_loaded(&mut self, id: Id) -> Option<bool> {
        let inst = self.insts.get(&id)?;
        find_prop(inst.class, "Image")?;
        let image = inst.get("Image").as_text().unwrap_or_default();
        let Some(file) = image.strip_prefix("rbxasset://horizon-studio/") else {
            return Some(false);
        };
        let dir = self.workspace_dir.clone();
        Some(self.screen_images.loads(&dir, file))
    }
}

/// `inst.key = value`.
fn newindex(env: &mut dyn Env, lua: &mut Lua, args: &[Value]) -> Result<(), Throw> {
    let id = inst_arg(env, lua, args, 0, "newindex")?;
    let key = args.get(1).and_then(Value::as_text).unwrap_or_default();
    let v = args.get(2).cloned().unwrap_or_default();
    let rbx = env.rbx();
    let c = rbx.inst(id).class;
    if rbx.inst(id).unscriptable.contains(&key) {
        return Err(lua.rt_error(format!(
            "{key} is not a valid member of {} \"{}\"",
            c.name,
            rbx.full_name(id)
        )));
    }
    let bad = |what: &str, v: &Value| {
        lua.rt_error(format!(
            "Unable to assign property {key}. {what} expected, got {}",
            lua.type_of(v)
        ))
    };
    match key.as_str() {
        "Name" => {
            let Some(name) = coerce(rbx, K::Str, &v) else {
                return Err(bad("string", &v));
            };
            if rbx.inst(id).locked && (c.service || id == rbx.game) {
                return Err(lua.rt_error(format!("Unable to change the Name of {}", c.name)));
            }
            let name = name.as_text().unwrap_or_default();
            if rbx.inst(id).name != name {
                rbx.inst_mut(id).name = name;
                rbx.changed(id, "Name", &Value::Nil);
                if let Some(p) = rbx.inst(id).parent {
                    rbx.child_arrived(p, id);
                }
            }
            return Ok(());
        }
        "Parent" => {
            let parent = match &v {
                Value::Nil => None,
                other => Some(rbx.id_of(other).ok_or_else(|| bad("Instance", other))?),
            };
            return rbx.set_parent(id, parent).map_err(|e| lua.rt_error(e));
        }
        "ClassName" => return Err(lua.rt_error("Unable to assign property ClassName. Property is read only")),
        _ => {}
    }
    if let Some(p) = find_prop(c, &key) {
        if p.ro {
            return Err(lua.rt_error(format!("Unable to assign property {key}. Property is read only")));
        }
        let value = coerce(rbx, p.k, &v).ok_or_else(|| bad(&kind_name(p.k), &v))?;
        rbx.raw_set(id, p.name, value);
        rbx.after_set(id, p.name);
        return Ok(());
    }
    match key.as_str() {
        "Position" | "Orientation" | "Rotation" if is_a(c, "BasePart") => {
            let x = as_v3(&v).ok_or_else(|| bad("Vector3", &v))?;
            let frame = as_cf(&rbx.raw_get(id, "CFrame")).unwrap_or_default();
            let r = |v: f64| v.to_radians();
            let next = match key.as_str() {
                "Position" => CFrame::from_position(x) * frame.rotation(),
                "Orientation" => {
                    CFrame::from_position(frame.position()) * CFrame::from_orientation(r(x.x), r(x.y), r(x.z))
                }
                _ => CFrame::from_position(frame.position()) * CFrame::angles(r(x.x), r(x.y), r(x.z)),
            };
            rbx.raw_set(id, "CFrame", cf(next));
            rbx.fire(id, &format!("prop:{key}"), Vec::new());
            rbx.after_set(id, "CFrame");
            Ok(())
        }
        "BrickColor" if is_a(c, "BasePart") => {
            let b = match get(&v) {
                Some(Data::Brick(b)) => b,
                _ => return Err(bad("BrickColor", &v)),
            };
            rbx.raw_set(id, "Color", c3(b.color()));
            Ok(())
        }
        "Font" if find_prop(c, "FontFace").is_some() => {
            let it = coerce_enum("Font", &v).ok_or_else(|| bad("Enum.Font", &v))?;
            let e = FontEnum::from_name(it.name).unwrap_or(FontEnum::Legacy);
            let face = Font::from_enum(e).unwrap_or_default();
            rbx.raw_set(id, "FontFace", wrap(Data::Font(face)));
            rbx.fire(id, "prop:Font", Vec::new());
            Ok(())
        }
        "Transparency" if is_a(c, "GuiObject") => {
            let value = coerce(rbx, K::Num, &v).ok_or_else(|| bad("number", &v))?;
            rbx.raw_set(id, "BackgroundTransparency", value);
            rbx.fire(id, "prop:Transparency", Vec::new());
            Ok(())
        }
        "ContentText" | "GuiState" | "IsLoaded"
            if match key.as_str() {
                "ContentText" => find_prop(c, "RichText").is_some(),
                "GuiState" => is_a(c, "GuiObject"),
                _ => find_prop(c, "Image").is_some(),
            } =>
        {
            Err(lua.rt_error(format!("Unable to assign property {key}. Property is read only")))
        }
        _ if c.lenient || rbx.inst(id).revealed.contains(&key) => {
            rbx.inst_mut(id).extra.insert(key, v);
            Ok(())
        }
        _ => Err(lua.rt_error(format!(
            "{key} is not a valid member of {} \"{}\"",
            c.name,
            rbx.full_name(id)
        ))),
    }
}

impl Rbx {
    /// What a script's assignment does beyond storing the value.
    fn after_set(&mut self, id: Id, prop: &str) {
        let class = self.inst(id).class.name;
        match (class, prop) {
            ("Sound", "Playing") if self.raw_get(id, "Playing").truthy() => self.play_sound(id),
            (_, "CFrame") if id == self.root_part => {
                let at = as_cf(&self.raw_get(id, "CFrame")).unwrap_or_default().position();
                self.sends
                    .push(format!("tp @s {:.3} {:.3} {:.3}", at.x, at.y - 0.9, at.z));
            }
            ("TextBox", "Text") => {}
            _ => {}
        }
    }

    pub fn play_sound(&mut self, id: Id) {
        let sid = self.raw_get(id, "SoundId").as_text().unwrap_or_default();
        // Roblox's asset ids mean nothing here; a sound event's name plays.
        let event = sid.trim();
        if event.is_empty() || event.starts_with("rbxasset") || event.chars().all(|c| c.is_ascii_digit()) {
            return;
        }
        let volume = self.raw_get(id, "Volume").to_num().unwrap_or(0.5) as f32;
        let pitch = self.raw_get(id, "PlaybackSpeed").to_num().unwrap_or(1.0) as f32;
        let at = self
            .inst(id)
            .parent
            .and_then(|p| as_cf(&self.raw_get(p, "CFrame")).map(|c| c.position()))
            .map(|p| [p.x, p.y, p.z]);
        self.sounds.push((
            event.to_owned(),
            at,
            volume.clamp(0.0, 10.0) * 2.0,
            pitch.clamp(0.5, 2.0),
        ));
        self.quiet_set(id, "IsPlaying", Value::Bool(true));
        self.fire(id, "Played", vec![Value::str(event)]);
    }

    /// A deep copy of `src` and what is under it (Archivable ones), for
    /// `owner`; references inside the copy point into the copy.
    pub fn clone_tree(&mut self, src: Id, owner: Owner, children: bool) -> Id {
        let mut map: HashMap<Id, Id> = HashMap::new();
        let root = self.copy_one(src, owner, &mut map, children);
        // Object properties that point inside the tree now point to the copies.
        let copies: Vec<Id> = map.values().copied().collect();
        for c in copies {
            let props: Vec<(&'static str, Value)> = self.inst(c).props.iter().map(|(k, v)| (*k, v.clone())).collect();
            for (k, v) in props {
                if let Some(old) = self.id_of(&v)
                    && let Some(&new) = map.get(&old)
                {
                    let nv = self.value(new);
                    self.quiet_set(c, k, nv);
                }
            }
        }
        root
    }

    fn copy_one(&mut self, src: Id, owner: Owner, map: &mut HashMap<Id, Id>, children: bool) -> Id {
        let (class, name, props, extra, attributes, tags, kids) = {
            let s = self.inst(src);
            (
                s.class.name,
                s.name.clone(),
                s.props.clone(),
                s.extra.clone(),
                s.attributes.clone(),
                s.tags.clone(),
                s.children.clone(),
            )
        };
        let id = self.make(class, Some(owner));
        {
            let n = self.inst_mut(id);
            n.name = name;
            n.props = props;
            n.extra = extra;
            n.attributes = attributes;
            n.tags = tags;
        }
        map.insert(src, id);
        if children || map.len() == 1 {
            for k in kids {
                if self.raw_get(k, "Archivable").truthy() && !self.inst(k).destroyed {
                    let ck = self.copy_one(k, owner, map, true);
                    let _ = self.set_parent(ck, Some(id));
                }
            }
        }
        id
    }

    /// Instances with `tag`.
    pub fn tagged(&self, tag: &str) -> Vec<Id> {
        let mut ids: Vec<Id> = self
            .insts
            .values()
            .filter(|i| !i.destroyed && i.tags.iter().any(|t| t == tag))
            .map(|i| i.id)
            .collect();
        ids.sort_unstable();
        ids
    }

    pub fn list(&self, ids: &[Id]) -> Value {
        Value::table(Table::from_list(ids.iter().map(|&i| self.value(i)).collect()))
    }

    pub fn tween_value(&self, id: Id) -> Option<&Tween> {
        self.tweens.iter().find(|t| t.id == id)
    }
}

/// Coerces a goal for a tween of `prop` on `id`.
pub fn tween_goal(rbx: &Rbx, id: Id, prop: &str, v: &Value) -> Result<&'static str, String> {
    let c = rbx.inst(id).class;
    let p = match prop {
        "Position" if is_a(c, "BasePart") => return Err("tween a BasePart's CFrame, not its Position".into()),
        "Name" => {
            return Err(format!(
                "TweenService:Create property named 'Name' cannot be tweened due to type mismatch (property is a 'string', but given type is '{}')",
                v.type_name()
            ));
        }
        // GuiObject's Transparency is its BackgroundTransparency.
        "Transparency" if is_a(c, "GuiObject") => find_prop(c, "BackgroundTransparency")
            .ok_or_else(|| format!("{prop} is not a valid member of {}", c.name))?,
        _ => find_prop(c, prop).ok_or_else(|| format!("{prop} is not a valid member of {}", c.name))?,
    };
    if !matches!(
        p.k,
        K::Num | K::Int | K::V2 | K::V3 | K::CF | K::C3 | K::UD | K::UD2 | K::Bool | K::Rect
    ) {
        return Err(format!(
            "TweenService:Create property named '{prop}' cannot be tweened due to type mismatch (property is a '{}', but given type is '{}')",
            kind_name(p.k),
            v.type_name()
        ));
    }
    coerce(rbx, p.k, v).ok_or_else(|| {
        format!(
            "TweenService:Create property named '{prop}' cannot be tweened due to type mismatch (property is a '{}')",
            kind_name(p.k)
        )
    })?;
    Ok(p.name)
}

/// The value between `a` and `b` at `t` (0 to 1), as a tween moves it.
pub fn lerp_value(a: &Value, b: &Value, t: f64) -> Value {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => Value::Num(x + (y - x) * t),
        (Value::Bool(_), _) => {
            if t >= 1.0 {
                b.clone()
            } else {
                a.clone()
            }
        }
        _ => match (get(a), get(b)) {
            (Some(Data::V2(x)), Some(Data::V2(y))) => v2(x.lerp(y, t)),
            (Some(Data::V3(x)), Some(Data::V3(y))) => v3(x.lerp(y, t)),
            (Some(Data::CF(x)), Some(Data::CF(y))) => cf(x.lerp(y, t)),
            (Some(Data::C3(x)), Some(Data::C3(y))) => c3(x.lerp(y, t)),
            (Some(Data::UD(x)), Some(Data::UD(y))) => udim(x.lerp(y, t)),
            (Some(Data::UD2(x)), Some(Data::UD2(y))) => udim2(x.lerp(y, t)),
            _ => {
                if t >= 1.0 {
                    b.clone()
                } else {
                    a.clone()
                }
            }
        },
    }
}

/// The eased progress of a tween at `elapsed` seconds into it, whether it
/// is over, and the time it takes in all (None: it repeats for ever).
pub fn tween_alpha(info: &TweenInfo, elapsed: f64) -> (f64, bool) {
    let t = elapsed - info.delay_time;
    if t <= 0.0 {
        return (0.0, false);
    }
    let cycle = info.time.max(0.0) * if info.reverses { 2.0 } else { 1.0 };
    if cycle <= 0.0 {
        return (1.0, true);
    }
    let cycles = if info.repeat_count < 0 {
        f64::INFINITY
    } else {
        f64::from(info.repeat_count) + 1.0
    };
    if t >= cycle * cycles {
        let end = if info.reverses { 0.0 } else { 1.0 };
        return (end, true);
    }
    let within = t % cycle;
    let a = if info.reverses && within > info.time {
        1.0 - (within - info.time) / info.time
    } else {
        within / info.time.max(1e-9)
    };
    (ease(info.easing_style, info.easing_direction, a.clamp(0.0, 1.0)), false)
}

/// Easing for GuiObject:TweenPosition's older arguments.
pub fn legacy_info(lua: &Lua, args: &[Value], at: usize, fname: &str) -> Result<TweenInfo, Throw> {
    let dir = match args.get(at) {
        None | Some(Value::Nil) => EasingDirection::Out,
        Some(v) => coerce_enum("EasingDirection", v)
            .and_then(|i| EasingDirection::from_name(i.name))
            .ok_or_else(|| bad_arg(lua, at, fname, "Enum.EasingDirection expected"))?,
    };
    let style = match args.get(at + 1) {
        None | Some(Value::Nil) => EasingStyle::Quad,
        Some(v) => coerce_enum("EasingStyle", v)
            .and_then(|i| EasingStyle::from_name(i.name))
            .ok_or_else(|| bad_arg(lua, at + 1, fname, "Enum.EasingStyle expected"))?,
    };
    let time = opt_num(lua, args, at + 2, fname, 1.0)?;
    Ok(TweenInfo::new(time, style, dir, 0, false, 0.0))
}

/// `Instance` and its constructors, for a mod's globals.
pub fn instance_table(rbx: &mut Rbx) -> Value {
    let mut t = Table::new();
    t.set_str("new", rbx.native(INSTANCE_NEW, "new"));
    t.set_str("fromExisting", rbx.native(INSTANCE_FROM_EXISTING, "fromExisting"));
    t.set_readonly(true);
    Value::table(t)
}

/// The host functions of the screen the GUI takes the pointer with.
pub fn screen_table(rbx: &mut Rbx) -> TableRef {
    let mut t = Table::new();
    t.set_str("canvas", Value::Bool(true));
    t.set_str("backdrop", Value::Bool(false));
    for (i, name) in [
        "draw", "move", "click", "release", "key", "text", "wheel", "close", "escape",
    ]
    .iter()
    .enumerate()
    {
        t.set_str(name, rbx.native(SCREEN_DRAW + i as u32, name));
    }
    Rc::new(RefCell::new(t))
}

/// Reads `n` numbers.
pub fn num_args(lua: &Lua, args: &[Value], from: usize, n: usize, fname: &str) -> Result<Vec<f64>, Throw> {
    (from..from + n).map(|i| check_num(lua, args, i, fname)).collect()
}

/// A UDim2 argument.
pub fn udim2_arg(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<UDim2, Throw> {
    args.get(i)
        .and_then(as_udim2)
        .ok_or_else(|| bad_arg(lua, i, fname, "UDim2 expected"))
}

pub fn tween_info_arg(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<TweenInfo, Throw> {
    args.get(i)
        .and_then(as_tween)
        .ok_or_else(|| bad_arg(lua, i, fname, "TweenInfo expected"))
}

pub fn udim_of(v: &Value) -> UDim {
    as_udim(v).unwrap_or_default()
}

pub fn v2_of(v: &Value) -> Vector2 {
    as_v2(v).unwrap_or_default()
}

pub fn item_of(v: &Value) -> Option<EnumItem> {
    as_item(v)
}
