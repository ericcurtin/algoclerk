// SPDX-License-Identifier: Apache-2.0
// Entry point of the iOS wrapper. The game itself is the Rust static library libalgoclerk_app.a;
// winit (inside the library) runs UIApplicationMain.
#import <UIKit/UIKit.h>

extern void main_rs(void);

int main(int argc, char *argv[]) {
    @autoreleasepool {
        main_rs();
    }
    return 0;
}
