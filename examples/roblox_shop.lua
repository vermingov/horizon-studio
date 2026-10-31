-- name: Roblox shop
-- version: 1.0
-- author: Horizon Studio
-- description: A Roblox UI script in Luau: a ScreenGui shop that slides in with TweenService. G (or /shop) opens it; click to buy.

local Players = game:GetService("Players")
local TweenService = game:GetService("TweenService")
local UserInputService = game:GetService("UserInputService")

type Item = { name: string, price: number, color: Color3 }

local ITEMS: { Item } = {
	{ name = "Diamond sword", price = 30, color = Color3.fromRGB(80, 200, 255) },
	{ name = "Golden apple", price = 12, color = Color3.fromRGB(255, 200, 60) },
	{ name = "Ender pearl", price = 8, color = Color3.fromRGB(60, 160, 120) },
}

local coins = 50
local open = false

local gui = Instance.new("ScreenGui")
gui.Name = "Shop"
gui.ResetOnSpawn = false
gui.Parent = Players.LocalPlayer:WaitForChild("PlayerGui")

local panel = Instance.new("Frame")
panel.Name = "Panel"
panel.Size = UDim2.fromOffset(260, 0)
panel.AutomaticSize = Enum.AutomaticSize.Y
panel.AnchorPoint = Vector2.new(0.5, 0.5)
panel.Position = UDim2.fromScale(0.5, 1.5)
panel.BackgroundColor3 = Color3.fromRGB(24, 26, 32)
panel.BorderSizePixel = 0
panel.Parent = gui
Instance.new("UICorner", panel).CornerRadius = UDim.new(0, 12)
local stroke = Instance.new("UIStroke", panel)
stroke.Color = Color3.fromRGB(70, 75, 90)
local pad = Instance.new("UIPadding", panel)
for _, side in { "PaddingTop", "PaddingBottom", "PaddingLeft", "PaddingRight" } do
	pad[side] = UDim.new(0, 12)
end
local list = Instance.new("UIListLayout", panel)
list.Padding = UDim.new(0, 8)
list.SortOrder = Enum.SortOrder.LayoutOrder

local title = Instance.new("TextLabel")
title.LayoutOrder = 0
title.Size = UDim2.new(1, 0, 0, 24)
title.BackgroundTransparency = 1
title.TextColor3 = Color3.new(1, 1, 1)
title.TextSize = 18
title.FontFace = Font.fromEnum(Enum.Font.GothamBold)
title.TextXAlignment = Enum.TextXAlignment.Left
title.Parent = panel

local function refresh()
	title.Text = `Shop  ·  {coins} coins`
end
refresh()

for i, item in ITEMS do
	local button = Instance.new("TextButton")
	button.Name = item.name
	button.LayoutOrder = i
	button.Size = UDim2.new(1, 0, 0, 36)
	button.BackgroundColor3 = Color3.fromRGB(40, 44, 54)
	button.BorderSizePixel = 0
	button.TextColor3 = item.color
	button.TextSize = 14
	button.Text = `{item.name}  —  {item.price}`
	button.Parent = panel
	Instance.new("UICorner", button)
	button.MouseButton1Click:Connect(function()
		if coins < item.price then
			button.Text = "Not enough coins"
			task.wait(1)
			button.Text = `{item.name}  —  {item.price}`
			return
		end
		coins -= item.price
		refresh()
		chat.show(`Bought {item.name}!`)
	end)
end

local close = Instance.new("TextButton")
close.LayoutOrder = #ITEMS + 1
close.Size = UDim2.new(1, 0, 0, 28)
close.BackgroundTransparency = 1
close.TextColor3 = Color3.fromRGB(160, 165, 180)
close.Text = "Close (G)"
close.Parent = panel

local function setOpen(on: boolean)
	open = on
	-- A Modal button frees the mouse while the shop shows.
	close.Modal = on
	local goal = if on then UDim2.fromScale(0.5, 0.5) else UDim2.fromScale(0.5, 1.5)
	TweenService:Create(panel, TweenInfo.new(0.35, Enum.EasingStyle.Quint, Enum.EasingDirection.Out), {
		Position = goal,
	}):Play()
end

close.MouseButton1Click:Connect(function()
	setOpen(false)
end)

UserInputService.InputBegan:Connect(function(input, processed)
	if not processed and input.KeyCode == Enum.KeyCode.G then
		setOpen(not open)
	end
end)

command("shop", function()
	setOpen(not open)
end, "Opens or closes the shop")
