https://partner.steamgames.com/doc/features/steam_controller/iga_file



(VDF is Valve's own data format for creating structured object data with key/value string pairs, not specific to steam input)

## Layout

- "In Game Actions". the type of the document. can not rename this scope
  - actions
    - each action set
  - localization

```vdf
"In Game Actions"
{
  "actions"
  {
    "ShipMovement"
    {
      "title"     "#ShipMovement"
      "Button"
      {
        "Fire"    "#fire"
      }
      "StickPadGyro"
      {
        "Move"
        {
          "title"         "#move"
          "input_mode"    "joystick_move"
        }
      }
    }
  }
  "localization"
  {
    "english"
    {
      "ShipMovement"    "Ship Movement"
      "move"            "Move"
      "fire"            "Fire"
    }
    "swedish"
    {
      "ShipMovement"    "Rymdskepp-kontroll"
      "move"            "Flytta"
      "fire"            "skjuta"
    }
  }
}
```

## Action types

There are only three different types:

- StickPadGyro. X and Y axis.
- AnalogTrigger. One axis.
- Button. True / False.

### StickPadGyro

has two different sub modes, (input_mode):
- joystick_move
- absolute_mouse


## Location of game action file

https://dev.epicgames.com/community/learning/tutorials/qM7o/unreal-engine-steamworks-input-api

for development, name it: `game_actions_[APPID].vdf` and place it in the `controller_config` sub directory of the steam directory:

- macos: `/Users/[MAC_USER_NAME]/Library/Application Support/Steam/controller_config`

`[SteamDirectory]\UserData\[youruserid]\241100\remote\controller_config\[appid]\[savename].vdf`

on macos it is typically:

`/Users/[MAC_USER_NAME]/Library/Application Support/Steam/userdata/[STEAM_USER_ID]/`
