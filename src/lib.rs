#![no_std]

//! This crate provides a ST7735 driver to connect to TFT displays.

pub mod instruction;

use core::mem::transmute;

use crate::instruction::Instruction;
use num_traits::ToPrimitive;
use num_derive::ToPrimitive;

use embedded_hal::digital::v2::OutputPin;
use embedded_hal::blocking::spi;
use embedded_hal::blocking::delay::DelayMs;

/// ST7735 driver to connect to TFT displays.
pub struct ST7735 <SPI, DC, RST>
where
    SPI: spi::Write<u8>,
    DC: OutputPin,
    RST: OutputPin,
{
    /// SPI
    spi: SPI,

    /// Data/command pin.
    dc: DC,

    /// Reset pin.
    rst: RST,

    /// Whether the display is RGB (true) or BGR (false)
    rgb: bool,

    /// Whether the colours are inverted (true) or not (false)
    inverted: bool,

    /// Global image offset
    dx: u16,
    dy: u16,
}

/// Display orientation.
#[derive(ToPrimitive)]
pub enum Orientation {
    Portrait = 0x00,
    Landscape = 0x60,
    PortraitSwapped = 0xC0,
    LandscapeSwapped = 0xA0,
}

impl<SPI, DC, RST> ST7735<SPI, DC, RST>
where
    SPI: spi::Write<u8>,
    DC: OutputPin,
    RST: OutputPin,
{
    /// Creates a new driver instance that uses hardware SPI.
    pub fn new(
        spi: SPI,
        dc: DC,
        rst: RST,
        rgb: bool,
        inverted: bool,
    ) -> Self
    {
        let display = ST7735 {
            spi,
            dc,
            rst,
            rgb,
            inverted,
            dx: 0,
            dy: 0
        };

        display
    }

    /// Runs commands to initialize the display.
    pub fn init<DELAY>(&mut self, delay: &mut DELAY) -> Result<(), ()>
        where DELAY: DelayMs<u8>
    {
        self.hard_reset()?;
        self.write_command(Instruction::SWRESET, None)?;
        delay.delay_ms(200);
        self.write_command(Instruction::SLPOUT, None)?;
        delay.delay_ms(200);
        self.write_command(Instruction::FRMCTR1, Some(&[0x01, 0x2C, 0x2D]))?;
        self.write_command(Instruction::FRMCTR2, Some(&[0x01, 0x2C, 0x2D]))?;
        self.write_command(Instruction::FRMCTR3,
            Some(&[0x01, 0x2C, 0x2D, 0x01, 0x2C, 0x2D]))?;
        self.write_command(Instruction::INVCTR, Some(&[0x07]))?;
        self.write_command(Instruction::PWCTR1, Some(&[0xA2, 0x02, 0x84]))?;
        self.write_command(Instruction::PWCTR2, Some(&[0xC5]))?;
        self.write_command(Instruction::PWCTR3, Some(&[0x0A, 0x00]))?;
        self.write_command(Instruction::PWCTR4, Some(&[0x8A, 0x2A]))?;
        self.write_command(Instruction::PWCTR5, Some(&[0x8A, 0xEE]))?;
        self.write_command(Instruction::VMCTR1, Some(&[0x0E]))?;
        if self.inverted {
            self.write_command(Instruction::INVON, None)?;
        } else {
            self.write_command(Instruction::INVOFF, None)?;
        }
        if self.rgb {
            self.write_command(Instruction::MADCTL, Some(&[0x00]))?;
        } else {
            self.write_command(Instruction::MADCTL, Some(&[0x08]))?;
        }
        self.write_command(Instruction::COLMOD, Some(&[0x05]))?;
        self.write_command(Instruction::DISPON, None)?;
        delay.delay_ms(200);
        Ok(())
    }

    pub fn hard_reset(&mut self) -> Result<(), ()>
    {
        self.rst.set_high().map_err(|_| ())?;
        self.rst.set_low().map_err(|_| ())?;
        self.rst.set_high().map_err(|_| ())
    }

    fn write_command(&mut self, command: Instruction, params: Option<&[u8]>) -> Result<(), ()> {
        self.dc.set_low().map_err(|_| ())?;
        self.spi.write(&[command.to_u8().unwrap()]).map_err(|_| ())?;
        if params.is_some() {
            self.write_data(params.unwrap())?;
        }
        Ok(())
    }

    fn write_data(&mut self, data: &[u8]) -> Result<(), ()> {
        self.dc.set_high().map_err(|_| ())?;
        self.spi.write(data).map_err(|_| ())
    }

    /// Writes a data word to the display.
    fn write_word(&mut self, value: u16) -> Result<(), ()> {
        let bytes: [u8; 2] = unsafe { transmute(value.to_be()) };
        self.write_data(&bytes)
    }

    pub fn set_orientation(&mut self, orientation: &Orientation) -> Result<(), ()> {
        if self.rgb {
            self.write_command(
                Instruction::MADCTL, Some(&[orientation.to_u8().unwrap()]
            ))?;
        } else {
            self.write_command(
                Instruction::MADCTL, Some(&[orientation.to_u8().unwrap() | 0x08 ]
            ))?;
        }
        Ok(())
    }

    /// Sets the global offset of the displayed image
    pub fn set_offset(&mut self, dx: u16, dy: u16) {
        self.dx = dx;
        self.dy = dy;
    }

    /// Sets the address window for the display.
    fn set_address_window(&mut self, sx: u16, sy: u16, ex: u16, ey: u16) -> Result<(), ()> {
        self.write_command(Instruction::CASET, None)?;
        self.write_word(sx + self.dx)?;
        self.write_word(ex + self.dx)?;
        self.write_command(Instruction::RASET, None)?;
        self.write_word(sy + self.dy)?;
        self.write_word(ey + self.dy)
    }

    /// Sets a pixel color at the given coords.
    pub fn set_pixel(&mut self, x: u16, y: u16, color: u16) -> Result <(), ()> {
        self.set_address_window(x, y, x, y)?;
        self.write_command(Instruction::RAMWR, None)?;
        self.write_word(color)
    }

    /// Writes pixel colors sequentially into the current drawing window
    pub fn write_pixels<P: IntoIterator<Item = u16>>(&mut self, colors: P) -> Result <(), ()> {
        self.write_command(Instruction::RAMWR, None)?;
        for color in colors {
            self.write_word(color)?;
        }
        Ok(())
    }

    /// Sets pixel colors at the given drawing window
    pub fn set_pixels<P: IntoIterator<Item = u16>>(&mut self, sx: u16, sy: u16, ex: u16, ey: u16, colors: P) -> Result <(), ()> {
        self.set_address_window(sx, sy, ex, ey)?;
        self.write_pixels(colors)
    }

    //////////////////////////////////////////////////////////

    /// Batch the pixels into rows
    fn pixels_by_row<T, P>(&mut self, item_pixels: T) -> RowIterator<P>
    where
        T: IntoIterator<Item = Pixel<Rgb565>>, 
        //P: Iterator<Item = Pixel<Rgb565>>, 
        {
        let iter = item_pixels.into_iter();
        RowIterator::<P> {
            pixels: iter,
            x: 0,
            y: 0,
            color: 0,
            first_pixel: true,
        }
        /*
        for Pixel(coord, color) in item_pixels {
            self.set_pixel(coord.0 as u16, coord.1 as u16, color.0).expect("pixel write failed");
        }
        */
    }
    
    /// Batch the rows into blocks, which are contiguous rows
    fn pixels_by_block<T>(&mut self, item_pixels: T)
    where
        T: IntoIterator<Item = Pixel<Rgb565>>, {
        /*
        for Pixel(coord, color) in item_pixels {
            self.set_pixel(coord.0 as u16, coord.1 as u16, color.0).expect("pixel write failed");
        }
        */
    }    
}

/*
    impl<C> IntoIterator for BatchPixels<C>
    where
        C: PixelColor,
    {
        type Item = Pixel<C>;
        type IntoIter = RowIterator<C>;

        fn into_iter(self) -> Self::IntoIter {
            RowIterator {
                top_left: self.top_left,
                bottom_right: self.bottom_right,
                style: self.style,
                p: self.top_left,
            }
        }
    }
*/

/// Iterator for each row in the pixel data
#[derive(Debug, Clone, Copy)]
pub struct RowIterator<P /* : Iterator<Item = Pixel<Rgb565>> */ > {
    pixels:      P,
    x:           u16,
    y:           u16,
    color:       u16,
    first_pixel: bool,
}

pub struct PixelRow {
    pub x_left:  u16,
    pub x_right: u16,
    pub y:       u16,
    pub colors:  u16, ////
}

impl<P /* : Iterator<Item = Pixel<Rgb565>> */ > Iterator for RowIterator<P> {
    type Item = PixelRow;

    fn next(&mut self) -> Option<Self::Item> {
        None
        /*
            // Don't render anything if the rectangle has no border or fill color.
            if self.style.stroke_color.is_none() && self.style.fill_color.is_none() {
                return None;
            }

            loop {
                let mut out = None;

                // Finished, i.e. we're below the rect
                if self.p.y > self.bottom_right.y {
                    break None;
                }

                let border_width = self.style.stroke_width as i32;
                let tl = self.top_left;
                let br = self.bottom_right;

                // Border
                if (
                    // Top border
                    (self.p.y >= tl.y && self.p.y < tl.y + border_width)
                // Bottom border
                || (self.p.y <= br.y && self.p.y > br.y - border_width)
                // Left border
                || (self.p.x >= tl.x && self.p.x < tl.x + border_width)
                // Right border
                || (self.p.x <= br.x && self.p.x > br.x - border_width)
                ) && self.style.stroke_color.is_some()
                {
                    out = Some(Pixel(
                        self.p,
                        self.style.stroke_color.expect("Expected stroke"),
                    ));
                }
                // Fill
                else if let Some(fill) = self.style.fill_color {
                    out = Some(Pixel(self.p, fill));
                }

                self.p.x += 1;

                // Reached end of row? Jump down one line
                if self.p.x > self.bottom_right.x {
                    self.p.x = self.top_left.x;
                    self.p.y += 1;
                }

                if out.is_some() {
                    break out;
                }
            }
        */
    }
}

///////////////////////////////////

#[cfg(feature = "graphics")]
extern crate embedded_graphics;
#[cfg(feature = "graphics")]
use self::embedded_graphics::{drawable::{Pixel, Dimensions}, pixelcolor::Rgb565, Drawing, SizedDrawing};

#[cfg(feature = "graphics")]
impl<SPI, DC, RST> Drawing<Rgb565> for ST7735<SPI, DC, RST>
where
    SPI: spi::Write<u8>,
    DC: OutputPin,
    RST: OutputPin,
{
    fn draw<T>(&mut self, item_pixels: T)
    where
        T: IntoIterator<Item = Pixel<Rgb565>>,
    {
        for Pixel(coord, color) in item_pixels {
            self.set_pixel(coord.0 as u16, coord.1 as u16, color.0).expect("pixel write failed");
        }
    }
}

#[cfg(feature = "graphics")]
impl<SPI, DC, RST> SizedDrawing<Rgb565> for ST7735<SPI, DC, RST>
where
    SPI: spi::Write<u8>,
    DC: OutputPin,
    RST: OutputPin,
{
    fn draw_sized<T>(&mut self, item_pixels: T)
    where
        T: IntoIterator<Item = Pixel<Rgb565>> + Dimensions,
    {
        // Get bounding box `Coord`s as `(u32, u32)`
        let top_left = item_pixels.top_left();
        let bottom_right = item_pixels.bottom_right();

        self.set_pixels(top_left.0 as u16, top_left.1 as u16,
                        bottom_right.0 as u16, bottom_right.1 as u16,
                        item_pixels.into_iter().map(|Pixel(_coord, color)| color.0)).expect("pixels write failed")
    }
}
