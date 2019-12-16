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

//////////////////////////////////////////////////////////

/// Batch the pixels into rows
fn to_rows<P>(pixels: P) -> RowIterator<P>
where
    P: Iterator<Item = Pixel<Rgb565>>, {
    RowIterator::<P> {
        pixels,
        x_left: 0,
        x_right: 0,
        y: 0,
        colors: RowColors::new(),
        first_pixel: true,
    }
}

/// Batch the rows into blocks, which are contiguous rows
fn to_blocks<R>(rows: R) -> BlockIterator<R>
where
    R: Iterator<Item = PixelRow>, {
    BlockIterator::<R> {
        rows,
        x_left: 0,
        x_right: 0,
        y_top: 0,
        y_bottom: 0,
        colors: BlockColors::new(),
        first_row: true,
    }
}    

/// Max number of pixels per row
type MaxRowSize = heapless::consts::U240;
/// Max number of rows per block
type MaxBlockSize = heapless::consts::U10;

/// Consecutive color words for a row
type RowColors = heapless::Vec::<u16, MaxRowSize>;
/// Consecutive color rows for a block
type BlockColors = heapless::Vec::<RowColors, MaxBlockSize>;

/// Iterator for each row in the pixel data
#[derive(Debug, Clone)]
pub struct RowIterator<P: Iterator<Item = Pixel<Rgb565>>> {
    pixels:      P,
    x_left:      u16,
    x_right:     u16,
    y:           u16,
    colors:      RowColors,
    first_pixel: bool,
}

/// Iterator for each block in the pixel data
#[derive(Debug, Clone)]
pub struct BlockIterator<R: Iterator<Item = PixelRow>> {
    rows:        R,
    x_left:      u16,
    x_right:     u16,
    y_top:       u16,
    y_bottom:    u16,
    colors:      BlockColors,
    first_row:   bool,
}

/// A row of contiguous pixels
pub struct PixelRow {
    pub x_left:  u16,
    pub x_right: u16,
    pub y:       u16,
    pub colors:  RowColors,
}

/// A block of contiguous row
pub struct PixelBlock {
    pub x_left:   u16,
    pub x_right:  u16,
    pub y_top:    u16,
    pub y_bottom: u16,
    pub colors:   BlockColors,
}

impl<P: Iterator<Item = Pixel<Rgb565>>> Iterator for RowIterator<P> {
    type Item = PixelRow;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.pixels.next() {
                None => {
                    if self.first_pixel {
                        return None;  //  No pixels to group
                    }                    
                    //  Else return previous pixels as row.
                    let row = PixelRow {
                        x_left: self.x_left,
                        x_right: self.x_right,
                        y: self.y,
                        colors: self.colors.clone(),
                    };
                    self.colors.clear();
                    self.first_pixel = true;
                    return Some(row);
                }
                Some(Pixel(coord, color)) => {
                    let x = coord.0 as u16;
                    let y = coord.1 as u16;
                    let color = color.0;
                    //  Save the first pixel as the row start and handle next pixel.
                    if self.first_pixel {
                        self.first_pixel = false;
                        self.x_left = x;
                        self.x_right = x;
                        self.y = y;
                        self.colors.clear();
                        self.colors.push(color)
                            .expect("never");
                        continue;
                    }
                    //  If this pixel is adjacent to the previous pixel, add to the row.
                    if x == self.x_right + 1 && y == self.y {
                        self.colors.push(color)
                            .expect("row overflow");
                        self.x_right = x;
                        continue;
                    }
                    //  Else return previous pixels as row.
                    let row = PixelRow {
                        x_left: self.x_left,
                        x_right: self.x_right,
                        y: self.y,
                        colors: self.colors.clone(),
                    };
                    self.x_left = x;
                    self.x_right = x;
                    self.y = y;
                    self.colors.clear();
                    self.colors.push(color)
                        .expect("never");
                    return Some(row);
                }
            }
        }
    }
}

impl<R: Iterator<Item = PixelRow>> Iterator for BlockIterator<R> {
    type Item = PixelBlock;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.rows.next() {
                None => {
                    if self.first_row {
                        return None;  //  No rows to group
                    }                    
                    //  Else return previous rows as block.
                    let row = PixelBlock {
                        x_left: self.x_left,
                        x_right: self.x_right,
                        y_top: self.y_top,
                        y_bottom: self.y_bottom,
                        colors: self.colors.clone(),
                    };
                    self.colors.clear();
                    self.first_row = true;
                    return Some(row);
                }
                Some(PixelRow { x_left, x_right, y, colors, .. }) => {
                    //  Save the first row as the block start and handle next block.
                    if self.first_row {
                        self.first_row = false;
                        self.x_left = x_left;
                        self.x_right = x_right;
                        self.y_top = y;
                        self.y_bottom = y;
                        self.colors.clear();
                        self.colors.push(colors)
                            .expect("never");
                        continue;
                    }
                    //  If this row is adjacent to the previous row and same size, add to the block.
                    if y == self.y_bottom + 1 && x_left == self.x_left && x_right == self.x_right {                        
                        //  Don't add row if too many rows in the block.
                        if self.colors.push(colors.clone()).is_ok() {
                            self.y_bottom = y;
                            continue;    
                        }
                    }
                    //  Else return previous rows as block.
                    let row = PixelBlock {
                        x_left: self.x_left,
                        x_right: self.x_right,
                        y_top: self.y_top,
                        y_bottom: self.y_bottom,
                        colors: self.colors.clone(),
                    };
                    self.x_left = x_left;
                    self.x_right = x_right;
                    self.y_top = y;
                    self.y_bottom = y;
                    self.colors.clear();
                    self.colors.push(colors)
                        .expect("never");
                    return Some(row);
                }
            }
        }
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
