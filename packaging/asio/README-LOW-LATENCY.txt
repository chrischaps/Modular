Soba: the low-latency Windows build
=============================================

This build can also run on an audio interface's ASIO driver, which talks to
the hardware directly: from input to output in 11-20 ms on a Focusrite
Scarlett 2i2, against 60 ms or so through Windows Audio. That's quick enough
to play a guitar or sing through Soba.

To use it, install your interface's ASIO driver from its maker, open
Soba, and under Output choose ASIO as the audio system. Until then it
runs on Windows Audio, like the standard build.

ASIO is a registered trademark of Steinberg Media Technologies GmbH.
ASIO Interface Technology by Steinberg Media Technologies GmbH.


Licence
-------

Soba's own source code is under the MIT licence. This build also
contains Steinberg's ASIO SDK (version 2.3.4), which Steinberg licenses
either under its proprietary ASIO licence or under the GNU General Public
License version 3. This build uses the GPL: as a whole, it is distributed
under the terms of the GNU GPL version 3, in GPL-3.0.txt beside this file.
Steinberg's licence for the SDK is in ASIO-SDK-LICENSE.txt, and Soba's
own MIT licence in LICENSE.

The complete corresponding source:

- Soba: https://github.com/chrischaps/Soba, at the tag this build's
  release is named for (and in that release's source archives).
- The ASIO SDK, exactly as built into this program: the
  asio-sdk-source.zip attached to the same release on GitHub.

This program comes with ABSOLUTELY NO WARRANTY, to the extent permitted by
applicable law; see sections 15 and 16 of the GPL.
