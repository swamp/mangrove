# Mangrove 🌳

A cool 2D game engine that combines the power of [Limnus](https://github.com/swamp/limnus), 
[Swamp](https://github.com/swamp/swamp) and [Swamp Script](https://github.com/swamp/script)
in one awesome package!

It is far from production ready at this point. But please check it out if you want!

## Overview

Mangrove is a 2D game engine built in Rust that integrates several key components:

- [**Swamp Script**]((https://github.com/swamp/script)) - A custom scripting language designed for games.
- [**Swamp**](https://github.com/swamp/swamp) - Game development framework
- [**Limnus**](https://github.com/swamp/limnus) - Core game engine functionality 

## 🚀 Installation

Download the executable from the [Releases])(https://github.com/swamp/mangrove/releases)

### MacOS

Due to Apple's security measures, you'll need to allow the executable:

```sh
xattr -cr ./swamp
```

### Install using cargo
If you want to build it yourself:

```bash
cargo install --path crates/mangrove/
```

## Examples

Execute `mangrove` while you are in this [`examples/`](examples/README.md) directory.

## About Contributions

This is an open source project with a single copyright holder.
While the code is publicly available under [LICENSE](LICENSE), I am not accepting external contributions at this time.

You are welcome to:
- Use the code according to the license terms
- Fork the project for your own use
- Report issues
- Provide feedback
- Share the project

If you have suggestions or find bugs, please feel free to open an issue for discussion. While I cannot 
accept pull requests, I value your feedback and engagement with the project.

Thank you for your understanding and interest in the project! 🙏


## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

_Copyright (c) 2024 Peter Bjorklund. All rights reserved._
