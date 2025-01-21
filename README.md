# Telegram Chat on ESP32

Developing pull-based Telegram Bot on ESP32 board with Rust.

## Resources

- HTTP Client: https://github.com/drogue-iot/reqwless
- TLS for HTTP: https://github.com/esp-rs/esp-mbedtls
- Impl Rust on ESP32 book: https://github.com/ImplFerris/esp32-book
- esp-rs book: https://docs.esp-rs.org/book/
- Embedded Rust (no_std) on Espressif: https://docs.esp-rs.org/no_std-training/
- ESP HAL examples: https://github.com/esp-rs/esp-hal/tree/main/examples
- https://github.com/esp-rs/no_std-training/blob/main/intro/http-client/src/main.rs

## Goals

Here are couple of things I need to make to work:
- Telegram Bot to respond to my commands in Telegram Chat
- Using display (for example output last command from telegram chat)
- Recording sound and sending it to the server/chat in real-time

Commands:
- /ip - returns public ip of my router
- /led - toggle blinking led on esp32
- /echo {message} - send back message
- /remind {seconds/minutes/hours} {message} - send message in x seconds/minutes/hours

Whenever we receive command we should blink 3 times the led and then return into initial state.
So steps look like this:
- save current state
- set low
- set high
- set low
- set high
- set low
- set high
- set low
- set saved state

Super fun idea: /say command will take next voice message and run it on esp32-coonnected sound thing.
I will be at university and then I will send voice to the home.


## Setup

Oh man, that's a bit a headache.
I am on Windows, so I will describe how to setup it on windows.

Cargo install:
- espup
- esg-generate

Install USB to UART Bridge drivers: https://www.silabs.com/developer-tools/usb-to-uart-bridge-vcp-drivers
to connect ESP32 to machine.


Runner in `config.toml`:
```
espflash flash --chip esp32 --port COM1 --monitor
```

I use COM1 port on Windows on my machine.


## Why

Doing web frontend becomes boring over time and meaningless.
I will continue to work as a freelancer web engineer (hire me).

But at certain point want to start more low level "company".
Doing drones or robots or some system engineering where you really can think.

And I saw this cool open position at Turing Pi for Rust Firmware Engineer (ESP32-S3),
which could be nice transition between web freelancing and own company.


## Problems

Man, for some reason espflash often can't connect to ESP32 to flash it.

1. First click reset on board while holding boot
2. Run flashing process while holding boot button on board
3. When you have seen message that it connected unhold boot and click reset


## Embedded Language

Just thoughts on crafting language specifically targeting embedded programming.

## Thoughts

Let's only allow static memory to be used. No dynamic allocations will be present.
You will need to set static (or compile-time known, later on this) size for allocated memory.
Because in embedded the risk of hitting memory limits is much more than on other environments.

We will be able to compute minimal memory required by our system during compilation.
Even stack size will be computable (if we disallow recursion, which I hate).

There MAY be option for percent-based size of arrays, like this `queue[80%]`.
It means: allocate 80% of left over memory to the queue variable.
It's experimental feature and I don't have strong opinion on it.
Here is graphics:

```
[ static memory ][ maximum stack ][ left over memory ]
                                  [   queue 80%  ]
```

### Constraints

Because of requirements like compile-time-known size memory,
there are some constraints

- no alloc (obviously)
- no recursion (can't calculate stack, maybe allow recursion with limited maximum deepness)
- pointers to functions must have sum type of all functions it can be and known at compile-time!

I would like consider to block recursion at all.
But there is neat cases to solve with it, that's why max deepness constraint exists.

Let me elaborate on pointers to functions. From what I know embedded developers avoid them as much as can.
But as with recursion there are neat cases where you want pointer to function.
Each function has requires stack size to be executed.
Therefore, to calculate stack size required for whole program we need to know stack size of all functions.
When using C-style pointers to function, we can't do that.
We need tagged union like pointer to function, where we define all functions it can point to at compile time.
Then the maximum size of stack among all defined functions will be consider as required stack size.
Btw, it will be more of a reference to function, not a pointer.
If this will not be possible, then we will remove such option.
And require engineers to use `if` statements and tagged union types for execution.


### Comptime

Compile time execution should be really powerful. That's all I can say.
For example, I may want to precompile whole BinaryTree.
I need to be able to use functions with much more memory during compilation.
But I don't want doing "you can dynamically alloc during compilation", because it will become compicated.
And I want to be able to reuse functions in both comptime and runtime.
So we may pass statically allocated memory as argument and then during comptime just throw big blob to it.